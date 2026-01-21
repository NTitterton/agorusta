use aws_sdk_apigatewaymanagement::Client as ApiGwClient;
use aws_sdk_apigatewaymanagement::primitives::Blob;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub author_id: String,
    pub author_username: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<Message>,
    pub has_more: bool,
    pub next_cursor: Option<i64>,
}

fn get_table(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        format!(
            "agorusta-{}-dev",
            name.to_lowercase().replace("_table", "s")
        )
    })
}

/// Verify that the channel exists and belongs to the given server
async fn verify_channel(
    db: &DynamoClient,
    server_id: &str,
    channel_id: &str,
) -> Result<(), (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("CHANNELS_TABLE"))
        .key("server_id", AttributeValue::S(server_id.to_string()))
        .key("id", AttributeValue::S(channel_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    if result.item().is_none() {
        return Err((404, "Channel not found".to_string()));
    }

    Ok(())
}

/// Check if user is a member of the server
async fn check_membership(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<(), (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("MEMBERS_TABLE"))
        .key("server_id", AttributeValue::S(server_id.to_string()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    if result.item().is_none() {
        return Err((403, "You are not a member of this server".to_string()));
    }

    Ok(())
}

/// Create a new message in a channel
pub async fn create_message(
    db: &DynamoClient,
    server_id: &str,
    channel_id: &str,
    user_id: &str,
    username: &str,
    body: &str,
) -> Result<Message, (u16, String)> {
    // Verify membership
    check_membership(db, server_id, user_id).await?;

    // Verify channel exists in this server
    verify_channel(db, server_id, channel_id).await?;

    // Parse request
    let req: CreateMessageRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    // Validate content
    let content = req.content.trim();
    if content.is_empty() {
        return Err((400, "Message content cannot be empty".to_string()));
    }
    if content.len() > 2000 {
        return Err((400, "Message content cannot exceed 2000 characters".to_string()));
    }

    let message = Message {
        id: Uuid::new_v4().to_string(),
        channel_id: channel_id.to_string(),
        author_id: user_id.to_string(),
        author_username: username.to_string(),
        content: content.to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // Store in DynamoDB
    db.put_item()
        .table_name(get_table("MESSAGES_TABLE"))
        .item("channel_id", AttributeValue::S(message.channel_id.clone()))
        .item("created_at", AttributeValue::N(message.created_at.to_string()))
        .item("id", AttributeValue::S(message.id.clone()))
        .item("author_id", AttributeValue::S(message.author_id.clone()))
        .item("author_username", AttributeValue::S(message.author_username.clone()))
        .item("content", AttributeValue::S(message.content.clone()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to save message: {}", e)))?;

    Ok(message)
}

/// List messages in a channel with pagination
pub async fn list_messages(
    db: &DynamoClient,
    server_id: &str,
    channel_id: &str,
    user_id: &str,
    limit: usize,
    before: Option<i64>,
) -> Result<MessagesResponse, (u16, String)> {
    // Verify membership
    check_membership(db, server_id, user_id).await?;

    // Verify channel exists
    verify_channel(db, server_id, channel_id).await?;

    // Clamp limit
    let limit = limit.min(100).max(1);

    // Build query
    let mut query = db
        .query()
        .table_name(get_table("MESSAGES_TABLE"))
        .key_condition_expression(if before.is_some() {
            "channel_id = :cid AND created_at < :before"
        } else {
            "channel_id = :cid"
        })
        .expression_attribute_values(":cid", AttributeValue::S(channel_id.to_string()))
        .scan_index_forward(false) // Newest first
        .limit((limit + 1) as i32); // Fetch one extra to check has_more

    if let Some(before_ts) = before {
        query = query.expression_attribute_values(":before", AttributeValue::N(before_ts.to_string()));
    }

    let result = query
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list messages: {}", e)))?;

    let mut messages: Vec<Message> = result
        .items()
        .iter()
        .filter_map(parse_message)
        .collect();

    // Check if there are more messages
    let has_more = messages.len() > limit;
    if has_more {
        messages.truncate(limit);
    }

    // Get cursor for next page (oldest message timestamp in this batch)
    let next_cursor = if has_more {
        messages.last().map(|m| m.created_at)
    } else {
        None
    };

    Ok(MessagesResponse {
        messages,
        has_more,
        next_cursor,
    })
}

fn parse_message(item: &HashMap<String, AttributeValue>) -> Option<Message> {
    Some(Message {
        id: item.get("id")?.as_s().ok()?.clone(),
        channel_id: item.get("channel_id")?.as_s().ok()?.clone(),
        author_id: item.get("author_id")?.as_s().ok()?.clone(),
        author_username: item.get("author_username")?.as_s().ok()?.clone(),
        content: item.get("content")?.as_s().ok()?.clone(),
        created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
    })
}

/// Broadcast a message to all WebSocket connections subscribed to the channel
pub async fn broadcast_message(
    db: &DynamoClient,
    apigw: &ApiGwClient,
    message: &Message,
) {
    // Find all connections subscribed to this channel
    let scan_result = db
        .scan()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .filter_expression("contains(channels, :channel_id)")
        .expression_attribute_values(
            ":channel_id",
            AttributeValue::S(message.channel_id.clone()),
        )
        .send()
        .await;

    let connections = match scan_result {
        Ok(result) => result.items().to_vec(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to scan connections");
            return;
        }
    };

    if connections.is_empty() {
        tracing::debug!(channel_id = %message.channel_id, "No subscribers for channel");
        return;
    }

    // Prepare broadcast payload
    let payload = serde_json::json!({
        "type": "new_message",
        "message": message
    });
    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize message");
            return;
        }
    };

    let num_recipients = connections.len();

    // Send to each connection
    for conn in &connections {
        let connection_id = match conn.get("connection_id").and_then(|v| v.as_s().ok()) {
            Some(id) => id.clone(),
            None => continue,
        };

        let result = apigw
            .post_to_connection()
            .connection_id(&connection_id)
            .data(Blob::new(payload_bytes.clone()))
            .send()
            .await;

        match result {
            Ok(_) => {
                tracing::debug!(connection_id = %connection_id, "Message sent");
            }
            Err(e) => {
                // Check if connection is stale (GoneException)
                let err_str = e.to_string();
                if err_str.contains("Gone") || err_str.contains("410") {
                    tracing::info!(connection_id = %connection_id, "Stale connection, removing");
                    // Delete stale connection
                    let _ = db
                        .delete_item()
                        .table_name(get_table("CONNECTIONS_TABLE"))
                        .key("connection_id", AttributeValue::S(connection_id))
                        .send()
                        .await;
                } else {
                    tracing::warn!(connection_id = %connection_id, error = %e, "Failed to send message");
                }
            }
        }
    }

    tracing::info!(
        channel_id = %message.channel_id,
        recipients = num_recipients,
        "Broadcast complete"
    );
}
