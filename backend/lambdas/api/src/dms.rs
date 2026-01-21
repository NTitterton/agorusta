use aws_sdk_apigatewaymanagement::primitives::Blob;
use aws_sdk_apigatewaymanagement::Client as ApiGwClient;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use uuid::Uuid;

// ============ Types ============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub other_user_id: String,
    pub other_username: String,
    pub updated_at: i64,
    pub last_message_preview: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessage {
    pub id: String,
    pub conversation_id: String,
    pub author_id: String,
    pub author_username: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct DmMessagesResponse {
    pub messages: Vec<DirectMessage>,
    pub has_more: bool,
    pub next_cursor: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct StartConversationRequest {
    pub recipient_id: String,
}

#[derive(Debug, Deserialize)]
pub struct SendDmRequest {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct UserSearchResult {
    pub id: String,
    pub username: String,
}

// ============ Helpers ============

fn get_table(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        format!(
            "agorusta-{}-dev",
            name.to_lowercase().replace("_table", "s")
        )
    })
}

/// Generate a deterministic conversation ID from two user IDs
fn make_conversation_id(user1: &str, user2: &str) -> String {
    let (min, max) = if user1 < user2 {
        (user1, user2)
    } else {
        (user2, user1)
    };
    format!("{}_{}", min, max)
}

/// Get user info by ID
async fn get_user_by_id(
    db: &DynamoClient,
    user_id: &str,
) -> Result<Option<(String, String)>, (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("USERS_TABLE"))
        .key("id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    Ok(result.item().and_then(|item| {
        let id = item.get("id")?.as_s().ok()?.clone();
        let username = item.get("username")?.as_s().ok()?.clone();
        Some((id, username))
    }))
}

/// Check if user is a participant in the conversation
async fn verify_participant(
    db: &DynamoClient,
    conversation_id: &str,
    user_id: &str,
) -> Result<Conversation, (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .key("id", AttributeValue::S(conversation_id.to_string()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    result
        .item()
        .and_then(parse_conversation)
        .ok_or((403, "You are not a participant in this conversation".to_string()))
}

fn parse_conversation(item: &HashMap<String, AttributeValue>) -> Option<Conversation> {
    Some(Conversation {
        id: item.get("id")?.as_s().ok()?.clone(),
        other_user_id: item.get("other_user_id")?.as_s().ok()?.clone(),
        other_username: item.get("other_username")?.as_s().ok()?.clone(),
        updated_at: item.get("updated_at")?.as_n().ok()?.parse().ok()?,
        last_message_preview: item
            .get("last_message_preview")
            .and_then(|v| v.as_s().ok().cloned()),
        created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
    })
}

fn parse_dm_message(item: &HashMap<String, AttributeValue>) -> Option<DirectMessage> {
    Some(DirectMessage {
        id: item.get("id")?.as_s().ok()?.clone(),
        conversation_id: item.get("conversation_id")?.as_s().ok()?.clone(),
        author_id: item.get("author_id")?.as_s().ok()?.clone(),
        author_username: item.get("author_username")?.as_s().ok()?.clone(),
        content: item.get("content")?.as_s().ok()?.clone(),
        created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
    })
}

// ============ User Search ============

pub async fn search_users(
    db: &DynamoClient,
    query: &str,
    current_user_id: &str,
) -> Result<Vec<UserSearchResult>, (u16, String)> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let query_lower = query.trim().to_lowercase();

    // Scan users table and filter by username prefix
    // Note: In production, you'd want a more efficient approach (e.g., ElasticSearch)
    // For now, we use a scan with filter since user count is small
    let result = db
        .scan()
        .table_name(get_table("USERS_TABLE"))
        .filter_expression("begins_with(username, :prefix) AND id <> :current_user")
        .expression_attribute_values(":prefix", AttributeValue::S(query_lower.clone()))
        .expression_attribute_values(":current_user", AttributeValue::S(current_user_id.to_string()))
        .limit(20)
        .send()
        .await
        .map_err(|e| (500, format!("Search failed: {}", e)))?;

    let users: Vec<UserSearchResult> = result
        .items()
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_s().ok()?.clone();
            let username = item.get("username")?.as_s().ok()?.clone();
            Some(UserSearchResult { id, username })
        })
        .collect();

    Ok(users)
}

// ============ Conversations ============

pub async fn list_conversations(
    db: &DynamoClient,
    user_id: &str,
) -> Result<Vec<Conversation>, (u16, String)> {
    let result = db
        .query()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .index_name("user-conversations-index")
        .key_condition_expression("user_id = :uid")
        .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
        .scan_index_forward(false) // Newest first
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list conversations: {}", e)))?;

    let conversations: Vec<Conversation> = result
        .items()
        .iter()
        .filter_map(parse_conversation)
        .collect();

    Ok(conversations)
}

pub async fn start_or_get_conversation(
    db: &DynamoClient,
    user_id: &str,
    username: &str,
    body: &str,
) -> Result<Conversation, (u16, String)> {
    let req: StartConversationRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    if req.recipient_id == user_id {
        return Err((400, "Cannot start a conversation with yourself".to_string()));
    }

    // Get recipient info
    let (recipient_id, recipient_username) = get_user_by_id(db, &req.recipient_id)
        .await?
        .ok_or((404, "User not found".to_string()))?;

    let conversation_id = make_conversation_id(user_id, &recipient_id);
    let now = chrono::Utc::now().timestamp_millis();

    // Check if conversation already exists for this user
    let existing = db
        .get_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .key("id", AttributeValue::S(conversation_id.clone()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    if let Some(item) = existing.item() {
        if let Some(conv) = parse_conversation(item) {
            return Ok(conv);
        }
    }

    // Create conversation records for both users
    // Record for current user
    db.put_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .item("id", AttributeValue::S(conversation_id.clone()))
        .item("user_id", AttributeValue::S(user_id.to_string()))
        .item("other_user_id", AttributeValue::S(recipient_id.clone()))
        .item("other_username", AttributeValue::S(recipient_username.clone()))
        .item("updated_at", AttributeValue::N(now.to_string()))
        .item("created_at", AttributeValue::N(now.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create conversation: {}", e)))?;

    // Record for recipient
    db.put_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .item("id", AttributeValue::S(conversation_id.clone()))
        .item("user_id", AttributeValue::S(recipient_id.clone()))
        .item("other_user_id", AttributeValue::S(user_id.to_string()))
        .item("other_username", AttributeValue::S(username.to_string()))
        .item("updated_at", AttributeValue::N(now.to_string()))
        .item("created_at", AttributeValue::N(now.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create conversation: {}", e)))?;

    Ok(Conversation {
        id: conversation_id,
        other_user_id: recipient_id,
        other_username: recipient_username,
        updated_at: now,
        last_message_preview: None,
        created_at: now,
    })
}

pub async fn get_conversation(
    db: &DynamoClient,
    conversation_id: &str,
    user_id: &str,
) -> Result<Conversation, (u16, String)> {
    verify_participant(db, conversation_id, user_id).await
}

// ============ Messages ============

pub async fn list_dm_messages(
    db: &DynamoClient,
    conversation_id: &str,
    user_id: &str,
    limit: usize,
    before: Option<i64>,
) -> Result<DmMessagesResponse, (u16, String)> {
    // Verify user is participant
    verify_participant(db, conversation_id, user_id).await?;

    let limit = limit.min(100).max(1);

    let mut query = db
        .query()
        .table_name(get_table("DM_MESSAGES_TABLE"))
        .key_condition_expression(if before.is_some() {
            "conversation_id = :cid AND created_at < :before"
        } else {
            "conversation_id = :cid"
        })
        .expression_attribute_values(":cid", AttributeValue::S(conversation_id.to_string()))
        .scan_index_forward(false)
        .limit((limit + 1) as i32);

    if let Some(before_ts) = before {
        query = query.expression_attribute_values(":before", AttributeValue::N(before_ts.to_string()));
    }

    let result = query
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list messages: {}", e)))?;

    let mut messages: Vec<DirectMessage> = result
        .items()
        .iter()
        .filter_map(parse_dm_message)
        .collect();

    let has_more = messages.len() > limit;
    if has_more {
        messages.truncate(limit);
    }

    let next_cursor = if has_more {
        messages.last().map(|m| m.created_at)
    } else {
        None
    };

    Ok(DmMessagesResponse {
        messages,
        has_more,
        next_cursor,
    })
}

pub async fn send_dm_message(
    db: &DynamoClient,
    conversation_id: &str,
    user_id: &str,
    username: &str,
    body: &str,
) -> Result<DirectMessage, (u16, String)> {
    // Verify user is participant
    let conversation = verify_participant(db, conversation_id, user_id).await?;

    let req: SendDmRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    let content = req.content.trim();
    if content.is_empty() {
        return Err((400, "Message content cannot be empty".to_string()));
    }
    if content.len() > 2000 {
        return Err((400, "Message content cannot exceed 2000 characters".to_string()));
    }

    let now = chrono::Utc::now().timestamp_millis();
    let message = DirectMessage {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.to_string(),
        author_id: user_id.to_string(),
        author_username: username.to_string(),
        content: content.to_string(),
        created_at: now,
    };

    // Store message
    db.put_item()
        .table_name(get_table("DM_MESSAGES_TABLE"))
        .item("conversation_id", AttributeValue::S(message.conversation_id.clone()))
        .item("created_at", AttributeValue::N(message.created_at.to_string()))
        .item("id", AttributeValue::S(message.id.clone()))
        .item("author_id", AttributeValue::S(message.author_id.clone()))
        .item("author_username", AttributeValue::S(message.author_username.clone()))
        .item("content", AttributeValue::S(message.content.clone()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to save message: {}", e)))?;

    // Update conversation records for both users
    let preview = if content.len() > 50 {
        format!("{}...", &content[..47])
    } else {
        content.to_string()
    };

    // Update current user's conversation record
    let _ = db
        .update_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .key("id", AttributeValue::S(conversation_id.to_string()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .update_expression("SET updated_at = :updated, last_message_preview = :preview")
        .expression_attribute_values(":updated", AttributeValue::N(now.to_string()))
        .expression_attribute_values(":preview", AttributeValue::S(preview.clone()))
        .send()
        .await;

    // Update other user's conversation record
    let _ = db
        .update_item()
        .table_name(get_table("DM_CONVERSATIONS_TABLE"))
        .key("id", AttributeValue::S(conversation_id.to_string()))
        .key("user_id", AttributeValue::S(conversation.other_user_id.clone()))
        .update_expression("SET updated_at = :updated, last_message_preview = :preview")
        .expression_attribute_values(":updated", AttributeValue::N(now.to_string()))
        .expression_attribute_values(":preview", AttributeValue::S(preview))
        .send()
        .await;

    Ok(message)
}

/// Broadcast a DM to WebSocket connections subscribed to the conversation
pub async fn broadcast_dm(db: &DynamoClient, apigw: &ApiGwClient, message: &DirectMessage) {
    // Find all connections subscribed to this conversation
    let scan_result = db
        .scan()
        .table_name(get_table("CONNECTIONS_TABLE"))
        .filter_expression("contains(channels, :conv_id)")
        .expression_attribute_values(
            ":conv_id",
            AttributeValue::S(message.conversation_id.clone()),
        )
        .send()
        .await;

    let connections = match scan_result {
        Ok(result) => result.items().to_vec(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to scan connections for DM");
            return;
        }
    };

    if connections.is_empty() {
        tracing::debug!(conversation_id = %message.conversation_id, "No subscribers for conversation");
        return;
    }

    let payload = serde_json::json!({
        "type": "new_dm",
        "message": message
    });
    let payload_bytes = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize DM");
            return;
        }
    };

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

        if let Err(e) = result {
            let err_str = e.to_string();
            if err_str.contains("Gone") || err_str.contains("410") {
                let _ = db
                    .delete_item()
                    .table_name(get_table("CONNECTIONS_TABLE"))
                    .key("connection_id", AttributeValue::S(connection_id))
                    .send()
                    .await;
            }
        }
    }

    tracing::info!(
        conversation_id = %message.conversation_id,
        "DM broadcast complete"
    );
}
