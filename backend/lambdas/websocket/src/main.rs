use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketEvent {
    request_context: RequestContext,
    body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestContext {
    connection_id: String,
    route_key: String,
    domain_name: Option<String>,
    stage: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketResponse {
    status_code: u16,
    body: Option<String>,
}

async fn handler(event: LambdaEvent<WebSocketEvent>) -> Result<WebSocketResponse, Error> {
    let (ws_event, _context) = event.into_parts();
    let connection_id = &ws_event.request_context.connection_id;
    let route_key = &ws_event.request_context.route_key;

    tracing::info!(
        connection_id = %connection_id,
        route_key = %route_key,
        "WebSocket event"
    );

    match route_key.as_str() {
        "$connect" => {
            tracing::info!(connection_id = %connection_id, "Client connected");
            // TODO: Store connection in DynamoDB
            Ok(WebSocketResponse {
                status_code: 200,
                body: None,
            })
        }
        "$disconnect" => {
            tracing::info!(connection_id = %connection_id, "Client disconnected");
            // TODO: Remove connection from DynamoDB
            Ok(WebSocketResponse {
                status_code: 200,
                body: None,
            })
        }
        "$default" | "message" => {
            if let Some(body) = &ws_event.body {
                tracing::info!(connection_id = %connection_id, body = %body, "Message received");
                // TODO: Process message and broadcast to channel
            }
            Ok(WebSocketResponse {
                status_code: 200,
                body: Some(r#"{"status":"received"}"#.to_string()),
            })
        }
        _ => {
            tracing::warn!(route_key = %route_key, "Unknown route");
            Ok(WebSocketResponse {
                status_code: 400,
                body: Some(r#"{"error":"unknown route"}"#.to_string()),
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    run(service_fn(handler)).await
}
