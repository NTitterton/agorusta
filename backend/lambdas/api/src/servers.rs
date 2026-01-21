use aws_sdk_dynamodb::Client as DynamoClient;
use aws_sdk_dynamodb::types::AttributeValue;
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub owner_id: String,
    pub icon_url: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub channel_type: String, // "text" or "voice"
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Member {
    pub server_id: String,
    pub user_id: String,
    pub username: String,
    pub role: String, // "owner", "admin", "member"
    pub joined_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    #[serde(default = "default_channel_type")]
    pub channel_type: String,
}

fn default_channel_type() -> String {
    "text".to_string()
}

#[derive(Debug, Serialize)]
pub struct ServerWithChannels {
    #[serde(flatten)]
    pub server: Server,
    pub channels: Vec<Channel>,
    pub member_count: usize,
}

fn get_table(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| format!("agorusta-{}-dev", name.to_lowercase().replace("_table", "s")))
}

// ============ Servers ============

pub async fn create_server(
    db: &DynamoClient,
    user_id: &str,
    username: &str,
    body: &str,
) -> Result<ServerWithChannels, (u16, String)> {
    let req: CreateServerRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err((400, "Server name must be 1-100 characters".to_string()));
    }

    let server_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    let server = Server {
        id: server_id.clone(),
        name: req.name.trim().to_string(),
        owner_id: user_id.to_string(),
        icon_url: None,
        created_at: now,
    };

    // Create the server
    db.put_item()
        .table_name(get_table("SERVERS_TABLE"))
        .item("id", AttributeValue::S(server.id.clone()))
        .item("name", AttributeValue::S(server.name.clone()))
        .item("owner_id", AttributeValue::S(server.owner_id.clone()))
        .item("created_at", AttributeValue::N(now.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create server: {}", e)))?;

    // Add creator as owner member
    let member = Member {
        server_id: server_id.clone(),
        user_id: user_id.to_string(),
        username: username.to_string(),
        role: "owner".to_string(),
        joined_at: now,
    };

    db.put_item()
        .table_name(get_table("MEMBERS_TABLE"))
        .item("server_id", AttributeValue::S(member.server_id.clone()))
        .item("user_id", AttributeValue::S(member.user_id.clone()))
        .item("username", AttributeValue::S(member.username.clone()))
        .item("role", AttributeValue::S(member.role.clone()))
        .item("joined_at", AttributeValue::N(now.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to add member: {}", e)))?;

    // Create default "general" channel
    let channel = Channel {
        id: Uuid::new_v4().to_string(),
        server_id: server_id.clone(),
        name: "general".to_string(),
        channel_type: "text".to_string(),
        created_at: now,
    };

    db.put_item()
        .table_name(get_table("CHANNELS_TABLE"))
        .item("server_id", AttributeValue::S(channel.server_id.clone()))
        .item("id", AttributeValue::S(channel.id.clone()))
        .item("name", AttributeValue::S(channel.name.clone()))
        .item("channel_type", AttributeValue::S(channel.channel_type.clone()))
        .item("created_at", AttributeValue::N(now.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create channel: {}", e)))?;

    Ok(ServerWithChannels {
        server,
        channels: vec![channel],
        member_count: 1,
    })
}

pub async fn list_user_servers(
    db: &DynamoClient,
    user_id: &str,
) -> Result<Vec<Server>, (u16, String)> {
    // Get all memberships for this user
    let memberships = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .index_name("user-servers-index")
        .key_condition_expression("user_id = :uid")
        .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list memberships: {}", e)))?;

    let server_ids: Vec<String> = memberships
        .items()
        .iter()
        .filter_map(|item| item.get("server_id")?.as_s().ok().cloned())
        .collect();

    if server_ids.is_empty() {
        return Ok(vec![]);
    }

    // Fetch each server (could batch this with BatchGetItem for optimization)
    let mut servers = Vec::new();
    for server_id in server_ids {
        if let Ok(result) = db
            .get_item()
            .table_name(get_table("SERVERS_TABLE"))
            .key("id", AttributeValue::S(server_id))
            .send()
            .await
        {
            if let Some(item) = result.item() {
                if let Some(server) = parse_server(item) {
                    servers.push(server);
                }
            }
        }
    }

    Ok(servers)
}

pub async fn get_server(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<ServerWithChannels, (u16, String)> {
    // Check membership
    check_membership(db, server_id, user_id).await?;

    // Get server
    let result = db
        .get_item()
        .table_name(get_table("SERVERS_TABLE"))
        .key("id", AttributeValue::S(server_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let server = result
        .item()
        .and_then(parse_server)
        .ok_or((404, "Server not found".to_string()))?;

    // Get channels
    let channels = list_channels(db, server_id).await?;

    // Get member count
    let members = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .select(aws_sdk_dynamodb::types::Select::Count)
        .send()
        .await
        .map_err(|e| (500, format!("Failed to count members: {}", e)))?;

    Ok(ServerWithChannels {
        server,
        channels,
        member_count: members.count() as usize,
    })
}

// ============ Channels ============

pub async fn create_channel(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
    body: &str,
) -> Result<Channel, (u16, String)> {
    // Check if user is owner or admin
    let role = get_member_role(db, server_id, user_id).await?;
    if role != "owner" && role != "admin" {
        return Err((403, "Only owners and admins can create channels".to_string()));
    }

    let req: CreateChannelRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err((400, "Channel name must be 1-100 characters".to_string()));
    }

    let channel = Channel {
        id: Uuid::new_v4().to_string(),
        server_id: server_id.to_string(),
        name: req.name.trim().to_lowercase().replace(' ', "-"),
        channel_type: req.channel_type,
        created_at: chrono::Utc::now().timestamp(),
    };

    db.put_item()
        .table_name(get_table("CHANNELS_TABLE"))
        .item("server_id", AttributeValue::S(channel.server_id.clone()))
        .item("id", AttributeValue::S(channel.id.clone()))
        .item("name", AttributeValue::S(channel.name.clone()))
        .item("channel_type", AttributeValue::S(channel.channel_type.clone()))
        .item("created_at", AttributeValue::N(channel.created_at.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create channel: {}", e)))?;

    Ok(channel)
}

pub async fn list_channels(
    db: &DynamoClient,
    server_id: &str,
) -> Result<Vec<Channel>, (u16, String)> {
    let result = db
        .query()
        .table_name(get_table("CHANNELS_TABLE"))
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list channels: {}", e)))?;

    let channels: Vec<Channel> = result
        .items()
        .iter()
        .filter_map(parse_channel)
        .collect();

    Ok(channels)
}

// ============ Members ============

pub async fn list_members(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<Vec<Member>, (u16, String)> {
    // Check membership
    check_membership(db, server_id, user_id).await?;

    let result = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list members: {}", e)))?;

    let members: Vec<Member> = result
        .items()
        .iter()
        .filter_map(parse_member)
        .collect();

    Ok(members)
}

// ============ Helpers ============

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

async fn get_member_role(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<String, (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("MEMBERS_TABLE"))
        .key("server_id", AttributeValue::S(server_id.to_string()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    result
        .item()
        .and_then(|item| item.get("role")?.as_s().ok().cloned())
        .ok_or((403, "You are not a member of this server".to_string()))
}

fn parse_server(item: &std::collections::HashMap<String, AttributeValue>) -> Option<Server> {
    Some(Server {
        id: item.get("id")?.as_s().ok()?.clone(),
        name: item.get("name")?.as_s().ok()?.clone(),
        owner_id: item.get("owner_id")?.as_s().ok()?.clone(),
        icon_url: item.get("icon_url").and_then(|v| v.as_s().ok().cloned()),
        created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
    })
}

fn parse_channel(item: &std::collections::HashMap<String, AttributeValue>) -> Option<Channel> {
    Some(Channel {
        id: item.get("id")?.as_s().ok()?.clone(),
        server_id: item.get("server_id")?.as_s().ok()?.clone(),
        name: item.get("name")?.as_s().ok()?.clone(),
        channel_type: item.get("channel_type")?.as_s().ok()?.clone(),
        created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
    })
}

fn parse_member(item: &std::collections::HashMap<String, AttributeValue>) -> Option<Member> {
    Some(Member {
        server_id: item.get("server_id")?.as_s().ok()?.clone(),
        user_id: item.get("user_id")?.as_s().ok()?.clone(),
        username: item.get("username")?.as_s().ok()?.clone(),
        role: item.get("role")?.as_s().ok()?.clone(),
        joined_at: item.get("joined_at")?.as_n().ok()?.parse().ok()?,
    })
}
