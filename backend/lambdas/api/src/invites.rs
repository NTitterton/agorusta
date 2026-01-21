use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

use crate::auth::{hash_password, verify_password};
use crate::servers::{Member, ServerWithChannels};

// ============ Types ============

#[derive(Debug, Serialize, Deserialize)]
pub struct Invite {
    pub code: String,
    pub server_id: String,
    pub server_name: String,
    pub created_by: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub max_uses: Option<i32>,
    pub use_count: i32,
}

#[derive(Debug, Serialize)]
pub struct InviteInfo {
    pub code: String,
    pub server_name: String,
    pub server_id: String,
    pub member_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub expires_in_hours: Option<i32>,
    pub max_uses: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerPassword {
    pub id: String,
    pub server_id: String,
    pub password_hash: String,
    pub created_by: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePasswordRequest {
    pub password: String,
    pub expires_in_hours: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct JoinByNameRequest {
    pub server_name: String,
    pub password: String,
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

fn generate_invite_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

async fn get_server_by_id(
    db: &DynamoClient,
    server_id: &str,
) -> Result<(String, String), (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("SERVERS_TABLE"))
        .key("id", AttributeValue::S(server_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let item = result.item().ok_or((404, "Server not found".to_string()))?;

    let name = item
        .get("name")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid server data".to_string()))?
        .clone();

    let owner_id = item
        .get("owner_id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid server data".to_string()))?
        .clone();

    Ok((name, owner_id))
}

async fn get_server_by_name(
    db: &DynamoClient,
    server_name: &str,
) -> Result<Option<(String, String)>, (u16, String)> {
    let result = db
        .query()
        .table_name(get_table("SERVERS_TABLE"))
        .index_name("name-index")
        .key_condition_expression("#n = :name")
        .expression_attribute_names("#n", "name")
        .expression_attribute_values(":name", AttributeValue::S(server_name.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let items = result.items();
    if items.is_empty() {
        return Ok(None);
    }

    let item = &items[0];
    let id = item
        .get("id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid server data".to_string()))?
        .clone();
    let owner_id = item
        .get("owner_id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid server data".to_string()))?
        .clone();

    Ok(Some((id, owner_id)))
}

async fn get_member_role(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<Option<String>, (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("MEMBERS_TABLE"))
        .key("server_id", AttributeValue::S(server_id.to_string()))
        .key("user_id", AttributeValue::S(user_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    Ok(result
        .item()
        .and_then(|item| item.get("role")?.as_s().ok().cloned()))
}

async fn count_members(db: &DynamoClient, server_id: &str) -> Result<usize, (u16, String)> {
    let result = db
        .query()
        .table_name(get_table("MEMBERS_TABLE"))
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .select(aws_sdk_dynamodb::types::Select::Count)
        .send()
        .await
        .map_err(|e| (500, format!("Failed to count members: {}", e)))?;

    Ok(result.count() as usize)
}

pub async fn add_member(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
    username: &str,
    role: &str,
) -> Result<Member, (u16, String)> {
    let now = chrono::Utc::now().timestamp();

    let member = Member {
        server_id: server_id.to_string(),
        user_id: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
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

    Ok(member)
}

// ============ Invite Functions ============

pub async fn create_invite(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
    body: &str,
) -> Result<Invite, (u16, String)> {
    // Check user is owner or admin
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" && role != "admin" {
        return Err((403, "Only owners and admins can create invites".to_string()));
    }

    let req: CreateInviteRequest = serde_json::from_str(body).unwrap_or(CreateInviteRequest {
        expires_in_hours: None,
        max_uses: None,
    });

    let (server_name, _) = get_server_by_id(db, server_id).await?;
    let now = chrono::Utc::now().timestamp();

    let expires_at = req
        .expires_in_hours
        .map(|h| now + (h as i64 * 3600));

    // Generate unique code with retry
    let mut code = generate_invite_code();
    let mut attempts = 0;
    loop {
        let mut put_builder = db
            .put_item()
            .table_name(get_table("INVITES_TABLE"))
            .item("code", AttributeValue::S(code.clone()))
            .item("server_id", AttributeValue::S(server_id.to_string()))
            .item("server_name", AttributeValue::S(server_name.clone()))
            .item("created_by", AttributeValue::S(user_id.to_string()))
            .item("created_at", AttributeValue::N(now.to_string()))
            .item("use_count", AttributeValue::N("0".to_string()));

        if let Some(exp) = expires_at {
            put_builder = put_builder
                .item("expires_at", AttributeValue::N(exp.to_string()))
                .item("ttl", AttributeValue::N(exp.to_string()));
        }

        if let Some(max) = req.max_uses {
            put_builder = put_builder.item("max_uses", AttributeValue::N(max.to_string()));
        }

        let result = put_builder
            .condition_expression("attribute_not_exists(code)")
            .send()
            .await;

        match result {
            Ok(_) => break,
            Err(e) => {
                if attempts >= 5 {
                    return Err((500, format!("Failed to create invite: {}", e)));
                }
                code = generate_invite_code();
                attempts += 1;
            }
        }
    }

    Ok(Invite {
        code,
        server_id: server_id.to_string(),
        server_name,
        created_by: user_id.to_string(),
        created_at: now,
        expires_at,
        max_uses: req.max_uses,
        use_count: 0,
    })
}

pub async fn list_invites(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<Vec<Invite>, (u16, String)> {
    // Check user is owner or admin
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" && role != "admin" {
        return Err((403, "Only owners and admins can view invites".to_string()));
    }

    let result = db
        .query()
        .table_name(get_table("INVITES_TABLE"))
        .index_name("server-invites-index")
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .scan_index_forward(false) // newest first
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list invites: {}", e)))?;

    let now = chrono::Utc::now().timestamp();
    let invites: Vec<Invite> = result
        .items()
        .iter()
        .filter_map(|item| {
            let expires_at = item
                .get("expires_at")
                .and_then(|v| v.as_n().ok()?.parse().ok());

            // Filter out expired invites
            if let Some(exp) = expires_at {
                if exp < now {
                    return None;
                }
            }

            Some(Invite {
                code: item.get("code")?.as_s().ok()?.clone(),
                server_id: item.get("server_id")?.as_s().ok()?.clone(),
                server_name: item
                    .get("server_name")
                    .and_then(|v| v.as_s().ok())
                    .cloned()
                    .unwrap_or_default(),
                created_by: item.get("created_by")?.as_s().ok()?.clone(),
                created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
                expires_at,
                max_uses: item
                    .get("max_uses")
                    .and_then(|v| v.as_n().ok()?.parse().ok()),
                use_count: item
                    .get("use_count")
                    .and_then(|v| v.as_n().ok()?.parse().ok())
                    .unwrap_or(0),
            })
        })
        .collect();

    Ok(invites)
}

pub async fn delete_invite(
    db: &DynamoClient,
    server_id: &str,
    code: &str,
    user_id: &str,
) -> Result<(), (u16, String)> {
    // Check user is owner or admin
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" && role != "admin" {
        return Err((403, "Only owners and admins can delete invites".to_string()));
    }

    // Verify invite belongs to this server
    let result = db
        .get_item()
        .table_name(get_table("INVITES_TABLE"))
        .key("code", AttributeValue::S(code.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let item = result
        .item()
        .ok_or((404, "Invite not found".to_string()))?;

    let invite_server_id = item
        .get("server_id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid invite data".to_string()))?;

    if invite_server_id != server_id {
        return Err((404, "Invite not found".to_string()));
    }

    db.delete_item()
        .table_name(get_table("INVITES_TABLE"))
        .key("code", AttributeValue::S(code.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to delete invite: {}", e)))?;

    Ok(())
}

pub async fn get_invite_info(db: &DynamoClient, code: &str) -> Result<InviteInfo, (u16, String)> {
    let result = db
        .get_item()
        .table_name(get_table("INVITES_TABLE"))
        .key("code", AttributeValue::S(code.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let item = result
        .item()
        .ok_or((404, "Invite not found or expired".to_string()))?;

    let now = chrono::Utc::now().timestamp();

    // Check if expired
    if let Some(expires_at) = item.get("expires_at").and_then(|v| v.as_n().ok()) {
        if let Ok(exp) = expires_at.parse::<i64>() {
            if exp < now {
                return Err((410, "This invite has expired".to_string()));
            }
        }
    }

    // Check max uses
    let use_count: i32 = item
        .get("use_count")
        .and_then(|v| v.as_n().ok()?.parse().ok())
        .unwrap_or(0);

    if let Some(max_uses) = item.get("max_uses").and_then(|v| v.as_n().ok()) {
        if let Ok(max) = max_uses.parse::<i32>() {
            if use_count >= max {
                return Err((410, "This invite has reached its usage limit".to_string()));
            }
        }
    }

    let server_id = item
        .get("server_id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid invite data".to_string()))?;

    let server_name = item
        .get("server_name")
        .and_then(|v| v.as_s().ok())
        .cloned()
        .unwrap_or_default();

    let member_count = count_members(db, server_id).await?;

    Ok(InviteInfo {
        code: code.to_string(),
        server_name,
        server_id: server_id.clone(),
        member_count,
    })
}

pub async fn join_by_code(
    db: &DynamoClient,
    code: &str,
    user_id: &str,
    username: &str,
) -> Result<ServerWithChannels, (u16, String)> {
    // Get and validate invite
    let invite_info = get_invite_info(db, code).await?;

    // Check if already a member
    if get_member_role(db, &invite_info.server_id, user_id)
        .await?
        .is_some()
    {
        return Err((409, "You are already a member of this server".to_string()));
    }

    // Increment use count
    db.update_item()
        .table_name(get_table("INVITES_TABLE"))
        .key("code", AttributeValue::S(code.to_string()))
        .update_expression("SET use_count = use_count + :inc")
        .expression_attribute_values(":inc", AttributeValue::N("1".to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to update invite: {}", e)))?;

    // Add member
    add_member(db, &invite_info.server_id, user_id, username, "member").await?;

    // Return server with channels
    crate::servers::get_server(db, &invite_info.server_id, user_id).await
}

// ============ Server Password Functions ============

pub async fn create_server_password(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
    body: &str,
) -> Result<ServerPassword, (u16, String)> {
    // Check user is owner
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" {
        return Err((
            403,
            "Only the server owner can create passwords".to_string(),
        ));
    }

    let req: CreatePasswordRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    if req.password.len() < 4 {
        return Err((400, "Password must be at least 4 characters".to_string()));
    }

    let password_hash =
        hash_password(&req.password).map_err(|e| (500, format!("Failed to hash password: {}", e)))?;

    let now = chrono::Utc::now().timestamp();
    let expires_at = req.expires_in_hours.map(|h| now + (h as i64 * 3600));
    let id = Uuid::new_v4().to_string();

    let mut put = db
        .put_item()
        .table_name(get_table("SERVER_PASSWORDS_TABLE"))
        .item("id", AttributeValue::S(id.clone()))
        .item("server_id", AttributeValue::S(server_id.to_string()))
        .item("password_hash", AttributeValue::S(password_hash.clone()))
        .item("created_by", AttributeValue::S(user_id.to_string()))
        .item("created_at", AttributeValue::N(now.to_string()));

    if let Some(exp) = expires_at {
        put = put
            .item("expires_at", AttributeValue::N(exp.to_string()))
            .item("ttl", AttributeValue::N(exp.to_string()));
    }

    put.send()
        .await
        .map_err(|e| (500, format!("Failed to create password: {}", e)))?;

    Ok(ServerPassword {
        id,
        server_id: server_id.to_string(),
        password_hash,
        created_by: user_id.to_string(),
        created_at: now,
        expires_at,
    })
}

pub async fn list_server_passwords(
    db: &DynamoClient,
    server_id: &str,
    user_id: &str,
) -> Result<Vec<ServerPassword>, (u16, String)> {
    // Check user is owner
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" {
        return Err((403, "Only the server owner can view passwords".to_string()));
    }

    let result = db
        .query()
        .table_name(get_table("SERVER_PASSWORDS_TABLE"))
        .index_name("server-passwords-index")
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.to_string()))
        .scan_index_forward(false)
        .send()
        .await
        .map_err(|e| (500, format!("Failed to list passwords: {}", e)))?;

    let now = chrono::Utc::now().timestamp();
    let passwords: Vec<ServerPassword> = result
        .items()
        .iter()
        .filter_map(|item| {
            let expires_at = item
                .get("expires_at")
                .and_then(|v| v.as_n().ok()?.parse().ok());

            // Filter out expired passwords
            if let Some(exp) = expires_at {
                if exp < now {
                    return None;
                }
            }

            Some(ServerPassword {
                id: item.get("id")?.as_s().ok()?.clone(),
                server_id: item.get("server_id")?.as_s().ok()?.clone(),
                password_hash: item.get("password_hash")?.as_s().ok()?.clone(),
                created_by: item.get("created_by")?.as_s().ok()?.clone(),
                created_at: item.get("created_at")?.as_n().ok()?.parse().ok()?,
                expires_at,
            })
        })
        .collect();

    Ok(passwords)
}

pub async fn delete_server_password(
    db: &DynamoClient,
    server_id: &str,
    password_id: &str,
    user_id: &str,
) -> Result<(), (u16, String)> {
    // Check user is owner
    let role = get_member_role(db, server_id, user_id)
        .await?
        .ok_or((403, "You are not a member of this server".to_string()))?;

    if role != "owner" {
        return Err((
            403,
            "Only the server owner can delete passwords".to_string(),
        ));
    }

    // Verify password belongs to this server
    let result = db
        .get_item()
        .table_name(get_table("SERVER_PASSWORDS_TABLE"))
        .key("id", AttributeValue::S(password_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let item = result
        .item()
        .ok_or((404, "Password not found".to_string()))?;

    let pwd_server_id = item
        .get("server_id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid password data".to_string()))?;

    if pwd_server_id != server_id {
        return Err((404, "Password not found".to_string()));
    }

    db.delete_item()
        .table_name(get_table("SERVER_PASSWORDS_TABLE"))
        .key("id", AttributeValue::S(password_id.to_string()))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to delete password: {}", e)))?;

    Ok(())
}

pub async fn join_by_name(
    db: &DynamoClient,
    body: &str,
    user_id: &str,
    username: &str,
) -> Result<ServerWithChannels, (u16, String)> {
    let req: JoinByNameRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request: {}", e)))?;

    // Find server by name
    let (server_id, _) = get_server_by_name(db, &req.server_name)
        .await?
        .ok_or((401, "Invalid server name or password".to_string()))?;

    // Check if already a member
    if get_member_role(db, &server_id, user_id).await?.is_some() {
        return Err((409, "You are already a member of this server".to_string()));
    }

    // Get all passwords for this server
    let result = db
        .query()
        .table_name(get_table("SERVER_PASSWORDS_TABLE"))
        .index_name("server-passwords-index")
        .key_condition_expression("server_id = :sid")
        .expression_attribute_values(":sid", AttributeValue::S(server_id.clone()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let now = chrono::Utc::now().timestamp();
    let mut password_matched = false;

    for item in result.items() {
        // Check if expired
        if let Some(expires_at) = item.get("expires_at").and_then(|v| v.as_n().ok()) {
            if let Ok(exp) = expires_at.parse::<i64>() {
                if exp < now {
                    continue; // Skip expired passwords
                }
            }
        }

        if let Some(hash) = item.get("password_hash").and_then(|v| v.as_s().ok()) {
            if verify_password(&req.password, hash) {
                password_matched = true;
                break;
            }
        }
    }

    if !password_matched {
        return Err((401, "Invalid server name or password".to_string()));
    }

    // Add member
    add_member(db, &server_id, user_id, username, "member").await?;

    // Return server with channels
    crate::servers::get_server(db, &server_id, user_id).await
}
