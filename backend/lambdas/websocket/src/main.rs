use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use jsonwebtoken::{decode, DecodingKey, Validation};
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketEvent {
    request_context: RequestContext,
    query_string_parameters: Option<QueryParams>,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestContext {
    connection_id: String,
    route_key: String,
    #[allow(dead_code)]
    domain_name: Option<String>,
    #[allow(dead_code)]
    stage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryParams {
    token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketResponse {
    status_code: u16,
    body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,  // user id
    email: String,
    exp: usize,
}

#[derive(Debug, Deserialize)]
struct WebSocketMessage {
    action: String,
    #[serde(default)]
    channel_id: Option<String>,
}

struct AppState {
    db: DynamoClient,
}

fn get_jwt_secret() -> String {
    env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
}

fn get_table(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        format!(
            "agorusta-{}-dev",
            name.to_lowercase().replace("_table", "s")
        )
    })
}

fn validate_token(token: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(get_jwt_secret().as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("Invalid token: {}", e))
}

async fn handle_connect(
    state: &AppState,
    connection_id: &str,
    query_params: &Option<QueryParams>,
) -> WebSocketResponse {
    // Extract and validate token from query params
    let token = match query_params.as_ref().and_then(|q| q.token.as_ref()) {
        Some(t) => t,
        None => {
            tracing::warn!(connection_id = %connection_id, "No token provided");
            return WebSocketResponse {
                status_code: 401,
                body: Some(r#"{"error":"unauthorized"}"#.to_string()),
            };
        }
    };

    let claims = match validate_token(token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(connection_id = %connection_id, error = %e, "Invalid token");
            return WebSocketResponse {
                status_code: 401,
                body: Some(r#"{"error":"unauthorized"}"#.to_string()),
            };
        }
    };

    // Store connection in DynamoDB with TTL (24 hours)
    let ttl = chrono::Utc::now().timestamp() + 86400;

    let result = state
        .db
        .put_item()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .item("connection_id", AttributeValue::S(connection_id.to_string()))
        .item("user_id", AttributeValue::S(claims.sub.clone()))
        .item("email", AttributeValue::S(claims.email.clone()))
        .item("channels", AttributeValue::Ss(vec![])) // Empty string set initially
        .item("ttl", AttributeValue::N(ttl.to_string()))
        .send()
        .await;

    match result {
        Ok(_) => {
            tracing::info!(
                connection_id = %connection_id,
                user_id = %claims.sub,
                "Client connected"
            );
            WebSocketResponse {
                status_code: 200,
                body: None,
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to store connection");
            WebSocketResponse {
                status_code: 500,
                body: Some(r#"{"error":"internal error"}"#.to_string()),
            }
        }
    }
}

async fn handle_disconnect(state: &AppState, connection_id: &str) -> WebSocketResponse {
    let result = state
        .db
        .delete_item()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .key("connection_id", AttributeValue::S(connection_id.to_string()))
        .send()
        .await;

    match result {
        Ok(_) => {
            tracing::info!(connection_id = %connection_id, "Client disconnected");
        }
        Err(e) => {
            tracing::error!(connection_id = %connection_id, error = %e, "Failed to remove connection");
        }
    }

    WebSocketResponse {
        status_code: 200,
        body: None,
    }
}

async fn handle_message(
    state: &AppState,
    connection_id: &str,
    body: &Option<String>,
) -> WebSocketResponse {
    let body_str = match body {
        Some(b) => b,
        None => {
            return WebSocketResponse {
                status_code: 400,
                body: Some(r#"{"error":"empty body"}"#.to_string()),
            };
        }
    };

    let msg: WebSocketMessage = match serde_json::from_str(body_str) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "Invalid message format");
            return WebSocketResponse {
                status_code: 400,
                body: Some(r#"{"error":"invalid message format"}"#.to_string()),
            };
        }
    };

    match msg.action.as_str() {
        "subscribe" => {
            let channel_id = match msg.channel_id {
                Some(c) => c,
                None => {
                    return WebSocketResponse {
                        status_code: 400,
                        body: Some(r#"{"error":"channel_id required"}"#.to_string()),
                    };
                }
            };

            // Add channel to connection's subscription list
            let result = state
                .db
                .update_item()
                .table_name(get_table("CONNECTIONS_TABLE"))
                .key("connection_id", AttributeValue::S(connection_id.to_string()))
                .update_expression("ADD channels :channel")
                .expression_attribute_values(
                    ":channel",
                    AttributeValue::Ss(vec![channel_id.clone()]),
                )
                .send()
                .await;

            match result {
                Ok(_) => {
                    tracing::info!(
                        connection_id = %connection_id,
                        channel_id = %channel_id,
                        "Subscribed to channel"
                    );
                    WebSocketResponse {
                        status_code: 200,
                        body: Some(
                            serde_json::json!({
                                "status": "subscribed",
                                "channel_id": channel_id
                            })
                            .to_string(),
                        ),
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to subscribe");
                    WebSocketResponse {
                        status_code: 500,
                        body: Some(r#"{"error":"failed to subscribe"}"#.to_string()),
                    }
                }
            }
        }
        "unsubscribe" => {
            let channel_id = match msg.channel_id {
                Some(c) => c,
                None => {
                    return WebSocketResponse {
                        status_code: 400,
                        body: Some(r#"{"error":"channel_id required"}"#.to_string()),
                    };
                }
            };

            // Remove channel from connection's subscription list
            let result = state
                .db
                .update_item()
                .table_name(get_table("CONNECTIONS_TABLE"))
                .key("connection_id", AttributeValue::S(connection_id.to_string()))
                .update_expression("DELETE channels :channel")
                .expression_attribute_values(
                    ":channel",
                    AttributeValue::Ss(vec![channel_id.clone()]),
                )
                .send()
                .await;

            match result {
                Ok(_) => {
                    tracing::info!(
                        connection_id = %connection_id,
                        channel_id = %channel_id,
                        "Unsubscribed from channel"
                    );
                    WebSocketResponse {
                        status_code: 200,
                        body: Some(
                            serde_json::json!({
                                "status": "unsubscribed",
                                "channel_id": channel_id
                            })
                            .to_string(),
                        ),
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to unsubscribe");
                    WebSocketResponse {
                        status_code: 500,
                        body: Some(r#"{"error":"failed to unsubscribe"}"#.to_string()),
                    }
                }
            }
        }
        _ => {
            tracing::warn!(action = %msg.action, "Unknown action");
            WebSocketResponse {
                status_code: 400,
                body: Some(r#"{"error":"unknown action"}"#.to_string()),
            }
        }
    }
}

async fn handler(
    event: LambdaEvent<WebSocketEvent>,
    state: &AppState,
) -> Result<WebSocketResponse, Error> {
    let (ws_event, _context) = event.into_parts();
    let connection_id = &ws_event.request_context.connection_id;
    let route_key = &ws_event.request_context.route_key;

    tracing::info!(
        connection_id = %connection_id,
        route_key = %route_key,
        "WebSocket event"
    );

    let response = match route_key.as_str() {
        "$connect" => handle_connect(state, connection_id, &ws_event.query_string_parameters).await,
        "$disconnect" => handle_disconnect(state, connection_id).await,
        "$default" => handle_message(state, connection_id, &ws_event.body).await,
        _ => {
            tracing::warn!(route_key = %route_key, "Unknown route");
            WebSocketResponse {
                status_code: 400,
                body: Some(r#"{"error":"unknown route"}"#.to_string()),
            }
        }
    };

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    // Initialize AWS SDK
    let config = aws_config::load_from_env().await;
    let db = DynamoClient::new(&config);
    let state = Arc::new(AppState { db });

    run(service_fn(move |event| {
        let state = Arc::clone(&state);
        async move { handler(event, &state).await }
    }))
    .await
}
