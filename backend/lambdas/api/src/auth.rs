use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::rngs::OsRng;
use aws_sdk_dynamodb::Client as DynamoClient;
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use std::env;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // user id
    pub email: String,
    pub username: String,
    pub exp: usize,   // expiration timestamp
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub username: String,
}

fn get_jwt_secret() -> String {
    env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-production".to_string())
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Failed to hash password: {}", e))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn create_token(user_id: &str, email: &str, username: &str) -> Result<String, String> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(7))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        username: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(get_jwt_secret().as_bytes()),
    )
    .map_err(|e| format!("Failed to create token: {}", e))
}

pub fn validate_token(token: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(get_jwt_secret().as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| format!("Invalid token: {}", e))
}

pub async fn register(
    db: &DynamoClient,
    body: &str,
) -> Result<AuthResponse, (u16, String)> {
    let req: RegisterRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request body: {}", e)))?;

    // Validate input
    if req.email.is_empty() || !req.email.contains('@') {
        return Err((400, "Invalid email".to_string()));
    }
    if req.username.len() < 3 {
        return Err((400, "Username must be at least 3 characters".to_string()));
    }
    if req.password.len() < 8 {
        return Err((400, "Password must be at least 8 characters".to_string()));
    }

    let table_name = env::var("USERS_TABLE").unwrap_or_else(|_| "agorusta-users-dev".to_string());

    // Check if email already exists
    let existing = db
        .query()
        .table_name(&table_name)
        .index_name("email-index")
        .key_condition_expression("email = :email")
        .expression_attribute_values(":email", aws_sdk_dynamodb::types::AttributeValue::S(req.email.clone()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    if existing.count() > 0 {
        return Err((409, "Email already registered".to_string()));
    }

    // Create user
    let user_id = Uuid::new_v4().to_string();
    let password_hash = hash_password(&req.password)
        .map_err(|e| (500, e))?;

    db.put_item()
        .table_name(&table_name)
        .item("id", aws_sdk_dynamodb::types::AttributeValue::S(user_id.clone()))
        .item("email", aws_sdk_dynamodb::types::AttributeValue::S(req.email.clone()))
        .item("username", aws_sdk_dynamodb::types::AttributeValue::S(req.username.clone()))
        .item("password_hash", aws_sdk_dynamodb::types::AttributeValue::S(password_hash))
        .send()
        .await
        .map_err(|e| (500, format!("Failed to create user: {}", e)))?;

    let token = create_token(&user_id, &req.email, &req.username)
        .map_err(|e| (500, e))?;

    Ok(AuthResponse {
        token,
        user: UserResponse {
            id: user_id,
            email: req.email,
            username: req.username,
        },
    })
}

pub async fn login(
    db: &DynamoClient,
    body: &str,
) -> Result<AuthResponse, (u16, String)> {
    let req: LoginRequest = serde_json::from_str(body)
        .map_err(|e| (400, format!("Invalid request body: {}", e)))?;

    let table_name = env::var("USERS_TABLE").unwrap_or_else(|_| "agorusta-users-dev".to_string());

    // Find user by email
    let result = db
        .query()
        .table_name(&table_name)
        .index_name("email-index")
        .key_condition_expression("email = :email")
        .expression_attribute_values(":email", aws_sdk_dynamodb::types::AttributeValue::S(req.email.clone()))
        .send()
        .await
        .map_err(|e| (500, format!("Database error: {}", e)))?;

    let items = result.items();
    if items.is_empty() {
        return Err((401, "Invalid email or password".to_string()));
    }

    let user = &items[0];

    let user_id = user.get("id")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid user data".to_string()))?;

    let username = user.get("username")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid user data".to_string()))?;

    let password_hash = user.get("password_hash")
        .and_then(|v| v.as_s().ok())
        .ok_or((500, "Invalid user data".to_string()))?;

    if !verify_password(&req.password, password_hash) {
        return Err((401, "Invalid email or password".to_string()));
    }

    let token = create_token(user_id, &req.email, username)
        .map_err(|e| (500, e))?;

    Ok(AuthResponse {
        token,
        user: UserResponse {
            id: user_id.clone(),
            email: req.email,
            username: username.clone(),
        },
    })
}
