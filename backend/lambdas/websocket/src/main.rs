use aws_sdk_apigatewaymanagement::{
    config::BehaviorVersion, primitives::Blob, Client as ApiGwClient,
};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use jsonwebtoken::{decode, DecodingKey, Validation};
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    username: String,
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
    apigw: Option<ApiGwClient>,
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

async fn broadcast_to_channel(
    state: &AppState,
    channel_id: &str,
    payload: &[u8],
    exclude_connection: Option<&str>,
) -> Result<(), String> {
    let apigw = match &state.apigw {
        Some(client) => client,
        None => return Err("API Gateway client not initialized".to_string()),
    };

    // Scan connections table for subscribers of this channel
    let scan_result = state
        .db
        .scan()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .filter_expression("contains(channels, :channel)")
        .expression_attribute_values(":channel", AttributeValue::S(channel_id.to_string()))
        .send()
        .await
        .map_err(|e| format!("Failed to scan connections: {}", e))?;

    let items = scan_result.items();

    for item in items {
        let conn_id = match item.get("connection_id").and_then(|v| v.as_s().ok()) {
            Some(id) => id,
            None => continue,
        };

        // Skip the sender
        if let Some(exclude) = exclude_connection {
            if conn_id == exclude {
                continue;
            }
        }

        // Send to this connection
        let send_result = apigw
            .post_to_connection()
            .connection_id(conn_id)
            .data(Blob::new(payload.to_vec()))
            .send()
            .await;

        if let Err(e) = send_result {
            tracing::warn!(
                connection_id = %conn_id,
                error = %e,
                "Failed to send to connection (may be stale)"
            );
        }
    }

    Ok(())
}

/// Get all server IDs that a user is a member of
async fn get_user_servers(db: &DynamoClient, user_id: &str) -> Vec<String> {
    let result = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .index_name("user-servers-index")
        .key_condition_expression("user_id = :uid")
        .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
        .send()
        .await;

    match result {
        Ok(output) => output
            .items()
            .iter()
            .filter_map(|item| item.get("server_id")?.as_s().ok().cloned())
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get user servers");
            vec![]
        }
    }
}

/// Get all members of a server (for broadcasting presence)
async fn get_server_member_ids(db: &DynamoClient, server_id: &str) -> Vec<String> {
    let result = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .send()
        .await;

    match result {
        Ok(output) => output
            .items()
            .iter()
            .filter_map(|item| item.get("user_id")?.as_s().ok().cloned())
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get server members");
            vec![]
        }
    }
}

/// Check if user has any other active connections
async fn user_has_other_connections(db: &DynamoClient, user_id: &str, exclude_connection_id: &str) -> bool {
    let result = db
        .query()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .index_name("user-connections-index")
        .key_condition_expression("user_id = :uid")
        .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
        .send()
        .await;

    match result {
        Ok(output) => {
            // Check if there are any connections other than the one being excluded
            output
                .items()
                .iter()
                .any(|item| {
                    item.get("connection_id")
                        .and_then(|v| v.as_s().ok())
                        .map(|id| id != exclude_connection_id)
                        .unwrap_or(false)
                })
        }
        Err(e) => {
            tracing::warn!(error = %e, "Failed to check user connections");
            false
        }
    }
}

/// Broadcast presence change to all members of the user's servers
async fn broadcast_presence_change(
    state: &AppState,
    user_id: &str,
    username: &str,
    is_online: bool,
) {
    let apigw = match &state.apigw {
        Some(client) => client,
        None => return,
    };

    // Get all servers the user is a member of
    let server_ids = get_user_servers(&state.db, user_id).await;

    // Collect all unique user IDs across all servers (to avoid duplicate broadcasts)
    let mut target_user_ids: HashSet<String> = HashSet::new();
    for server_id in &server_ids {
        let member_ids = get_server_member_ids(&state.db, server_id).await;
        target_user_ids.extend(member_ids);
    }

    // Remove the user themselves from the target list
    target_user_ids.remove(user_id);

    if target_user_ids.is_empty() {
        return;
    }

    // Build presence change payload
    let payload = serde_json::json!({
        "type": "presence_change",
        "user_id": user_id,
        "username": username,
        "is_online": is_online
    });
    let payload_bytes = serde_json::to_vec(&payload).unwrap();

    // Find all connections for target users and send the presence update
    for target_user_id in target_user_ids {
        // Query connections for this user
        let conn_result = state
            .db
            .query()
            .table_name(get_table("CONNECTIONS_TABLE"))
            .index_name("user-connections-index")
            .key_condition_expression("user_id = :uid")
            .expression_attribute_values(":uid", AttributeValue::S(target_user_id.clone()))
            .send()
            .await;

        if let Ok(output) = conn_result {
            for item in output.items() {
                if let Some(conn_id) = item.get("connection_id").and_then(|v| v.as_s().ok()) {
                    let send_result = apigw
                        .post_to_connection()
                        .connection_id(conn_id)
                        .data(Blob::new(payload_bytes.clone()))
                        .send()
                        .await;

                    if let Err(e) = send_result {
                        tracing::warn!(
                            connection_id = %conn_id,
                            error = %e,
                            "Failed to send presence update"
                        );
                    }
                }
            }
        }
    }

    tracing::info!(
        user_id = %user_id,
        is_online = %is_online,
        server_count = %server_ids.len(),
        "Broadcast presence change"
    );
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

    // Note: Don't set empty channels - DynamoDB doesn't allow empty String Sets
    // channels will be added via UPDATE when user subscribes
    let result = state
        .db
        .put_item()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .item("connection_id", AttributeValue::S(connection_id.to_string()))
        .item("user_id", AttributeValue::S(claims.sub.clone()))
        .item("email", AttributeValue::S(claims.email.clone()))
        .item("username", AttributeValue::S(claims.username.clone()))
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

            // Check if this is the user's first connection (they were previously offline)
            // We just added a connection, so if there's only one, they just came online
            let was_offline = !user_has_other_connections(&state.db, &claims.sub, connection_id).await;
            if was_offline {
                // Broadcast that user is now online
                broadcast_presence_change(state, &claims.sub, &claims.username, true).await;
            }

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
    // First, get the user info before deleting the connection
    let conn_result = state
        .db
        .get_item()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .key("connection_id", AttributeValue::S(connection_id.to_string()))
        .send()
        .await;

    let user_info = conn_result.ok().and_then(|output| {
        output.item.map(|item| {
            let user_id = item
                .get("user_id")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            let username = item
                .get("username")
                .and_then(|v| v.as_s().ok())
                .cloned()
                .unwrap_or_default();
            (user_id, username)
        })
    });

    // Delete the connection
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

            // Check if user has any other connections
            if let Some((user_id, username)) = user_info {
                if !user_id.is_empty() {
                    // Check if this was their last connection
                    let has_other = user_has_other_connections(&state.db, &user_id, connection_id).await;
                    if !has_other {
                        // Broadcast that user is now offline
                        broadcast_presence_change(state, &user_id, &username, false).await;
                    }
                }
            }
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
            let table_name = get_table("CONNECTIONS_TABLE");
            let result = state
                .db
                .update_item()
                .table_name(&table_name)
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
                        table = %table_name,
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
        "typing" | "stop_typing" => {
            let channel_id = match msg.channel_id {
                Some(c) => c,
                None => {
                    return WebSocketResponse {
                        status_code: 400,
                        body: Some(r#"{"error":"channel_id required"}"#.to_string()),
                    };
                }
            };

            // Get user info from connection
            let conn_result = state
                .db
                .get_item()
                .table_name(get_table("CONNECTIONS_TABLE"))
                .key("connection_id", AttributeValue::S(connection_id.to_string()))
                .send()
                .await;

            let (user_id, username) = match conn_result {
                Ok(output) => {
                    if let Some(item) = output.item {
                        let user_id = item
                            .get("user_id")
                            .and_then(|v| v.as_s().ok())
                            .cloned()
                            .unwrap_or_default();
                        let username = item
                            .get("username")
                            .and_then(|v| v.as_s().ok())
                            .cloned()
                            .unwrap_or_default();
                        (user_id, username)
                    } else {
                        return WebSocketResponse {
                            status_code: 404,
                            body: Some(r#"{"error":"connection not found"}"#.to_string()),
                        };
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to get connection");
                    return WebSocketResponse {
                        status_code: 500,
                        body: Some(r#"{"error":"internal error"}"#.to_string()),
                    };
                }
            };

            // Build typing event payload
            let is_typing = msg.action == "typing";
            let payload = serde_json::json!({
                "type": if is_typing { "user_typing" } else { "user_stop_typing" },
                "channel_id": channel_id,
                "user_id": user_id,
                "username": username
            });
            let payload_bytes = serde_json::to_vec(&payload).unwrap();

            // Broadcast to all subscribers of this channel (except sender)
            if let Err(e) = broadcast_to_channel(
                state,
                &channel_id,
                &payload_bytes,
                Some(connection_id),
            )
            .await
            {
                tracing::error!(error = %e, "Failed to broadcast typing");
            }

            WebSocketResponse {
                status_code: 200,
                body: None,
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

    // Initialize API Gateway Management client for broadcasting
    let apigw = if let Ok(endpoint) = env::var("WEBSOCKET_ENDPOINT") {
        let apigw_config = aws_sdk_apigatewaymanagement::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .credentials_provider(config.credentials_provider().unwrap().clone())
            .region(config.region().cloned())
            .endpoint_url(endpoint)
            .build();
        Some(ApiGwClient::from_conf(apigw_config))
    } else {
        tracing::warn!("WEBSOCKET_ENDPOINT not set, typing broadcasts disabled");
        None
    };

    let state = Arc::new(AppState { db, apigw });

    run(service_fn(move |event| {
        let state = Arc::clone(&state);
        async move { handler(event, &state).await }
    }))
    .await
}
