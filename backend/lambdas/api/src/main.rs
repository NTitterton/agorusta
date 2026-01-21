use aws_sdk_apigatewaymanagement::Client as ApiGwClient;
use aws_sdk_dynamodb::Client as DynamoClient;
use lambda_http::{run, service_fn, Body, Error, Request, RequestExt, Response};
use std::env;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod auth;
mod dms;
mod invites;
mod messages;
mod servers;

struct AppState {
    db: DynamoClient,
    apigw: Option<ApiGwClient>,
}

fn cors_response(status: u16, body: impl Into<Body>) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", "*")
        .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS")
        .header("access-control-allow-headers", "Content-Type, Authorization")
        .body(body.into())?)
}

fn json_response<T: serde::Serialize>(status: u16, data: &T) -> Result<Response<Body>, Error> {
    let body = serde_json::to_string(data).unwrap_or_else(|_| r#"{"error":"serialization error"}"#.to_string());
    cors_response(status, body)
}

fn error_response(status: u16, message: &str) -> Result<Response<Body>, Error> {
    cors_response(status, format!(r#"{{"error":"{}"}}"#, message))
}

fn get_auth(event: &Request) -> Option<auth::Claims> {
    let auth_header = event
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())?;

    let token = auth_header.strip_prefix("Bearer ")?;
    auth::validate_token(token).ok()
}

fn require_auth(event: &Request) -> Result<auth::Claims, Response<Body>> {
    get_auth(event).ok_or_else(|| {
        Response::builder()
            .status(401)
            .header("content-type", "application/json")
            .header("access-control-allow-origin", "*")
            .body(Body::from(r#"{"error":"unauthorized"}"#))
            .unwrap()
    })
}

async fn handler(event: Request, state: Arc<AppState>) -> Result<Response<Body>, Error> {
    let raw_path = event.uri().path();
    let method = event.method().as_str();

    // Strip stage prefix (e.g., /dev, /prod) from path
    let path = raw_path
        .strip_prefix("/dev")
        .or_else(|| raw_path.strip_prefix("/prod"))
        .unwrap_or(raw_path);
    let path = if path.is_empty() { "/" } else { path };

    tracing::info!(path = %path, method = %method, "Handling request");

    // Handle CORS preflight
    if method == "OPTIONS" {
        return cors_response(200, "");
    }

    // Get request body for POST/PUT requests
    let body = match event.body() {
        Body::Text(s) => s.clone(),
        Body::Binary(b) => String::from_utf8_lossy(b).to_string(),
        Body::Empty => String::new(),
    };

    // Parse path segments for dynamic routes
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    match (method, segments.as_slice()) {
        // Health check
        ("GET", ["health"]) => {
            cors_response(200, r#"{"status":"ok"}"#)
        }

        // ============ Auth routes ============
        ("POST", ["auth", "register"]) => {
            match auth::register(&state.db, &body).await {
                Ok(response) => json_response(201, &response),
                Err((status, message)) => error_response(status, &message),
            }
        }
        ("POST", ["auth", "login"]) => {
            match auth::login(&state.db, &body).await {
                Ok(response) => json_response(200, &response),
                Err((status, message)) => error_response(status, &message),
            }
        }
        ("GET", ["auth", "me"]) => {
            match require_auth(&event) {
                Ok(claims) => json_response(200, &serde_json::json!({
                    "id": claims.sub,
                    "email": claims.email
                })),
                Err(resp) => Ok(resp),
            }
        }

        // ============ Server routes ============
        ("GET", ["servers"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match servers::list_user_servers(&state.db, &claims.sub).await {
                        Ok(servers) => json_response(200, &servers),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["servers"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match servers::create_server(&state.db, &claims.sub, &claims.username, &body).await {
                        Ok(server) => json_response(201, &server),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["servers", server_id]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match servers::get_server(&state.db, server_id, &claims.sub).await {
                        Ok(server) => json_response(200, &server),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Channel routes ============
        ("GET", ["servers", server_id, "channels"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    // First check membership
                    match servers::get_server(&state.db, server_id, &claims.sub).await {
                        Ok(server) => json_response(200, &server.channels),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["servers", server_id, "channels"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match servers::create_channel(&state.db, server_id, &claims.sub, &body).await {
                        Ok(channel) => json_response(201, &channel),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Member routes ============
        ("GET", ["servers", server_id, "members"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match servers::list_members(&state.db, server_id, &claims.sub).await {
                        Ok(members) => json_response(200, &members),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Message routes ============
        ("GET", ["servers", server_id, "channels", channel_id, "messages"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    // Parse query params for pagination
                    let query_params = event.query_string_parameters();
                    let limit: usize = query_params
                        .first("limit")
                        .and_then(|v: &str| v.parse().ok())
                        .unwrap_or(50);
                    let before: Option<i64> = query_params
                        .first("before")
                        .and_then(|v: &str| v.parse().ok());

                    match messages::list_messages(
                        &state.db,
                        server_id,
                        channel_id,
                        &claims.sub,
                        limit,
                        before,
                    )
                    .await
                    {
                        Ok(response) => json_response(200, &response),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["servers", server_id, "channels", channel_id, "messages"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match messages::create_message(
                        &state.db,
                        server_id,
                        channel_id,
                        &claims.sub,
                        &claims.username,
                        &body,
                    )
                    .await
                    {
                        Ok(message) => {
                            // Broadcast to WebSocket subscribers (fire and forget)
                            if let Some(apigw) = &state.apigw {
                                messages::broadcast_message(&state.db, apigw, &message).await;
                            }
                            json_response(201, &message)
                        }
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Invite routes ============
        ("POST", ["servers", server_id, "invites"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::create_invite(&state.db, server_id, &claims.sub, &body).await {
                        Ok(invite) => json_response(201, &invite),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["servers", server_id, "invites"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::list_invites(&state.db, server_id, &claims.sub).await {
                        Ok(invites_list) => json_response(200, &invites_list),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("DELETE", ["servers", server_id, "invites", code]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::delete_invite(&state.db, server_id, code, &claims.sub).await {
                        Ok(()) => cors_response(204, ""),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["invites", code]) => {
            match require_auth(&event) {
                Ok(_) => {
                    match invites::get_invite_info(&state.db, code).await {
                        Ok(info) => json_response(200, &info),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["invites", code, "join"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::join_by_code(&state.db, code, &claims.sub, &claims.username).await {
                        Ok(server) => json_response(200, &server),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Server Password routes ============
        ("POST", ["servers", server_id, "passwords"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::create_server_password(&state.db, server_id, &claims.sub, &body).await {
                        Ok(password) => {
                            // Don't return the hash to the client
                            json_response(201, &serde_json::json!({
                                "id": password.id,
                                "server_id": password.server_id,
                                "created_at": password.created_at,
                                "expires_at": password.expires_at
                            }))
                        }
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["servers", server_id, "passwords"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::list_server_passwords(&state.db, server_id, &claims.sub).await {
                        Ok(passwords) => {
                            // Don't return hashes to the client
                            let safe_passwords: Vec<_> = passwords.iter().map(|p| {
                                serde_json::json!({
                                    "id": p.id,
                                    "server_id": p.server_id,
                                    "created_at": p.created_at,
                                    "expires_at": p.expires_at
                                })
                            }).collect();
                            json_response(200, &safe_passwords)
                        }
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("DELETE", ["servers", server_id, "passwords", password_id]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::delete_server_password(&state.db, server_id, password_id, &claims.sub).await {
                        Ok(()) => cors_response(204, ""),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ Join by name route ============
        ("POST", ["servers", "join"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match invites::join_by_name(&state.db, &body, &claims.sub, &claims.username).await {
                        Ok(server) => json_response(200, &server),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ User search route ============
        ("GET", ["users", "search"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    let query_params = event.query_string_parameters();
                    let query = query_params.first("q").unwrap_or("");
                    match dms::search_users(&state.db, query, &claims.sub).await {
                        Ok(users) => json_response(200, &users),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // ============ DM routes ============
        ("GET", ["dms"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match dms::list_conversations(&state.db, &claims.sub).await {
                        Ok(conversations) => json_response(200, &conversations),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["dms"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match dms::start_or_get_conversation(&state.db, &claims.sub, &claims.username, &body).await {
                        Ok(conversation) => json_response(201, &conversation),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["dms", conversation_id]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match dms::get_conversation(&state.db, conversation_id, &claims.sub).await {
                        Ok(conversation) => json_response(200, &conversation),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("GET", ["dms", conversation_id, "messages"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    let query_params = event.query_string_parameters();
                    let limit: usize = query_params
                        .first("limit")
                        .and_then(|v: &str| v.parse().ok())
                        .unwrap_or(50);
                    let before: Option<i64> = query_params
                        .first("before")
                        .and_then(|v: &str| v.parse().ok());

                    match dms::list_dm_messages(&state.db, conversation_id, &claims.sub, limit, before).await {
                        Ok(response) => json_response(200, &response),
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }
        ("POST", ["dms", conversation_id, "messages"]) => {
            match require_auth(&event) {
                Ok(claims) => {
                    match dms::send_dm_message(&state.db, conversation_id, &claims.sub, &claims.username, &body).await {
                        Ok(message) => {
                            // Broadcast to WebSocket subscribers
                            if let Some(apigw) = &state.apigw {
                                dms::broadcast_dm(&state.db, apigw, &message).await;
                            }
                            json_response(201, &message)
                        }
                        Err((status, message)) => error_response(status, &message),
                    }
                }
                Err(resp) => Ok(resp),
            }
        }

        // 404 for everything else
        _ => {
            error_response(404, "not found")
        }
    }
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

    // Initialize API Gateway Management client for WebSocket broadcasts
    let apigw = if let Ok(endpoint) = env::var("WEBSOCKET_ENDPOINT") {
        let apigw_config = aws_sdk_apigatewaymanagement::Config::builder()
            .endpoint_url(endpoint)
            .region(config.region().cloned())
            .credentials_provider(config.credentials_provider().unwrap().clone())
            .behavior_version(aws_sdk_apigatewaymanagement::config::BehaviorVersion::latest())
            .build();
        Some(ApiGwClient::from_conf(apigw_config))
    } else {
        tracing::warn!("WEBSOCKET_ENDPOINT not set, broadcast disabled");
        None
    };

    let state = Arc::new(AppState { db, apigw });

    run(service_fn(move |event| {
        let state = Arc::clone(&state);
        async move { handler(event, state).await }
    }))
    .await
}
