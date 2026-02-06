use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadRequest {
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub upload_url: String,
    pub file_id: String,
    pub download_url: String,
}

fn get_bucket() -> String {
    env::var("UPLOADS_BUCKET").unwrap_or_else(|_| "agorusta-uploads-dev".to_string())
}

// Allowed content types for upload
fn is_allowed_content_type(content_type: &str) -> bool {
    let allowed_prefixes = ["image/", "video/", "audio/", "application/pdf"];
    let allowed_exact = [
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "text/plain",
    ];

    allowed_prefixes.iter().any(|p| content_type.starts_with(p))
        || allowed_exact.contains(&content_type)
}

// Max file size: 50MB
const MAX_FILE_SIZE: i64 = 50 * 1024 * 1024;

pub async fn create_upload(
    s3: &S3Client,
    user_id: &str,
    request: UploadRequest,
) -> Result<UploadResponse, (u16, String)> {
    // Validate content type
    if !is_allowed_content_type(&request.content_type) {
        return Err((400, "File type not allowed".to_string()));
    }

    // Validate file size
    if request.size > MAX_FILE_SIZE {
        return Err((400, format!("File too large. Maximum size is {}MB", MAX_FILE_SIZE / 1024 / 1024)));
    }

    if request.size <= 0 {
        return Err((400, "Invalid file size".to_string()));
    }

    // Sanitize filename
    let safe_filename = request
        .filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect::<String>();

    if safe_filename.is_empty() {
        return Err((400, "Invalid filename".to_string()));
    }

    let bucket = get_bucket();
    let file_id = Uuid::new_v4().to_string();
    let key = format!("{}/{}/{}", user_id, file_id, safe_filename);

    // Generate presigned PUT URL (15 minutes)
    let presign_config = PresigningConfig::builder()
        .expires_in(Duration::from_secs(900))
        .build()
        .map_err(|e| (500, format!("Presigning config error: {}", e)))?;

    let upload_url = s3
        .put_object()
        .bucket(&bucket)
        .key(&key)
        .content_type(&request.content_type)
        .presigned(presign_config)
        .await
        .map_err(|e| (500, format!("Failed to generate upload URL: {}", e)))?
        .uri()
        .to_string();

    // Generate presigned GET URL (7 days)
    let download_presign_config = PresigningConfig::builder()
        .expires_in(Duration::from_secs(7 * 24 * 60 * 60))
        .build()
        .map_err(|e| (500, format!("Presigning config error: {}", e)))?;

    let download_url = s3
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .presigned(download_presign_config)
        .await
        .map_err(|e| (500, format!("Failed to generate download URL: {}", e)))?
        .uri()
        .to_string();

    Ok(UploadResponse {
        upload_url,
        file_id,
        download_url,
    })
}

pub async fn get_download_url(
    s3: &S3Client,
    user_id: &str,
    file_id: &str,
    filename: &str,
) -> Result<String, (u16, String)> {
    let bucket = get_bucket();
    let key = format!("{}/{}/{}", user_id, file_id, filename);

    let presign_config = PresigningConfig::builder()
        .expires_in(Duration::from_secs(3600)) // 1 hour
        .build()
        .map_err(|e| (500, format!("Presigning config error: {}", e)))?;

    let url = s3
        .get_object()
        .bucket(&bucket)
        .key(&key)
        .presigned(presign_config)
        .await
        .map_err(|e| (500, format!("Failed to generate download URL: {}", e)))?
        .uri()
        .to_string();

    Ok(url)
}
