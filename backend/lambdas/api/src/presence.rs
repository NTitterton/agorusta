use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DynamoClient;
use std::collections::HashSet;
use std::env;

fn get_table(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| {
        format!(
            "agorusta-{}-dev",
            name.to_lowercase().replace("_table", "s")
        )
    })
}

/// Check if a user has any active WebSocket connections
pub async fn is_user_online(db: &DynamoClient, user_id: &str) -> bool {
    let table = get_table("CONNECTIONS_TABLE");

    // Scan with filter - for small connection tables this is fine
    // For production scale, consider a different approach
    let result = db
        .scan()
        .table_name(&table)
        .filter_expression("user_id = :uid")
        .expression_attribute_values(":uid", AttributeValue::S(user_id.to_string()))
        .send()
        .await;

    match result {
        Ok(output) => {
            let is_online = !output.items().is_empty();
            tracing::info!(user_id = %user_id, is_online = %is_online, count = %output.items().len(), "Presence check result");
            is_online
        }
        Err(e) => {
            tracing::error!(error = %e, user_id = %user_id, "Failed to check user online status");
            false
        }
    }
}

/// Get online status for multiple users (batch)
/// Returns a HashSet of user IDs that are currently online
pub async fn get_online_users(db: &DynamoClient, user_ids: &[String]) -> HashSet<String> {
    let mut online_users = HashSet::new();

    // Query each user's connections (could be optimized with BatchGetItem if needed)
    for user_id in user_ids {
        if is_user_online(db, user_id).await {
            online_users.insert(user_id.clone());
        }
    }

    online_users
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_table_with_env_var() {
        std::env::set_var("CONNECTIONS_TABLE", "test-connections-table");
        assert_eq!(get_table("CONNECTIONS_TABLE"), "test-connections-table");
        std::env::remove_var("CONNECTIONS_TABLE");
    }

    #[test]
    fn test_get_table_default() {
        std::env::remove_var("SOME_TABLE");
        let result = get_table("SOME_TABLE");
        assert!(result.contains("agorusta"));
        assert!(result.contains("dev"));
    }
}
