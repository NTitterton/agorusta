use aws_sdk_dynamodb::Client as DynamoClient;
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

mod auth;
mod servers;

struct AppState {
    db: DynamoClient,
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
                    // We need username for member record - fetch from users table or include in token
                    // For now, use email prefix as username fallback
                    let username = claims.email.split('@').next().unwrap_or("user");
                    match servers::create_server(&state.db, &claims.sub, username, &body).await {
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
    let state = Arc::new(AppState { db });

    run(service_fn(move |event| {
        let state = Arc::clone(&state);
        async move { handler(event, state).await }
    }))
    .await
}
