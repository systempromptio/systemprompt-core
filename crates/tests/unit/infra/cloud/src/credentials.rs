//! Unit tests for CloudCredentials

use base64::prelude::*;
use chrono::{Duration, Utc};
use systemprompt_cloud::CloudCredentials;
use tempfile::TempDir;

fn create_valid_token(exp_offset_secs: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let exp = Utc::now().timestamp() + exp_offset_secs;
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("test_signature");
    format!("{}.{}.{}", header, payload, signature)
}

#[test]
fn test_cloud_credentials_new() {
    let creds = CloudCredentials::new(
        "test_token".to_string(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );

    assert_eq!(creds.api_token, "test_token");
    assert_eq!(creds.api_url, "https://api.test.io");
    assert_eq!(creds.user_email, "test@example.com");
}

#[test]
fn test_cloud_credentials_authenticated_at_is_now() {
    let before = Utc::now();
    let creds = CloudCredentials::new(
        "token".to_string(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );
    let after = Utc::now();

    assert!(creds.authenticated_at >= before);
    assert!(creds.authenticated_at <= after);
}

#[test]
fn test_cloud_credentials_token() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(token.clone(), "https://api.test.io".to_string(), "test@example.com".to_string());

    let cloud_token = creds.token();
    assert_eq!(cloud_token.as_str(), &token);
}

#[test]
fn test_cloud_credentials_is_token_expired_false_for_valid() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    assert!(!creds.is_token_expired());
}

#[test]
fn test_cloud_credentials_is_token_expired_true_for_expired() {
    let token = create_valid_token(-3600);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    assert!(creds.is_token_expired());
}

#[test]
fn test_cloud_credentials_expires_within_true_when_expiring_soon() {
    let token = create_valid_token(1800);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    assert!(creds.expires_within(Duration::hours(1)));
}

#[test]
fn test_cloud_credentials_expires_within_false_when_not_expiring_soon() {
    let token = create_valid_token(7200);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    assert!(!creds.expires_within(Duration::hours(1)));
}

#[test]
fn test_cloud_credentials_serialization() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        token.clone(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );

    let json = serde_json::to_string(&creds).unwrap();
    assert!(json.contains(&token));
    assert!(json.contains("https://api.test.io"));
    assert!(json.contains("test@example.com"));
    assert!(json.contains("authenticated_at"));
}

#[test]
fn test_cloud_credentials_serialization_includes_email() {
    let creds = CloudCredentials::new(
        "token".to_string(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );

    let json = serde_json::to_string(&creds).unwrap();
    assert!(json.contains("user_email"));
    assert!(json.contains("test@example.com"));
}

#[test]
fn test_cloud_credentials_deserialization() {
    let json = r#"{
        "api_token": "test_token",
        "api_url": "https://api.test.io",
        "authenticated_at": "2024-01-15T12:00:00Z",
        "user_email": "test@example.com"
    }"#;

    let creds: CloudCredentials = serde_json::from_str(json).unwrap();
    assert_eq!(creds.api_token, "test_token");
    assert_eq!(creds.api_url, "https://api.test.io");
    assert_eq!(creds.user_email, "test@example.com");
}

#[test]
fn test_cloud_credentials_roundtrip() {
    let token = create_valid_token(3600);
    let original = CloudCredentials::new(
        token,
        "https://api.systemprompt.io".to_string(),
        "admin@example.com".to_string(),
    );

    let json = serde_json::to_string(&original).unwrap();
    let restored: CloudCredentials = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.api_token, original.api_token);
    assert_eq!(restored.api_url, original.api_url);
    assert_eq!(restored.user_email, original.user_email);
}

#[test]
fn test_cloud_credentials_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        token.clone(),
        "https://api.systemprompt.io".to_string(),
        "test@example.com".to_string(),
    );

    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());

    let loaded = CloudCredentials::load_from_path(&creds_path).unwrap();
    assert_eq!(loaded.api_token, token);
    assert_eq!(loaded.api_url, "https://api.systemprompt.io");
}

#[test]
fn test_cloud_credentials_save_creates_parent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("nested").join("dir").join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());
}

#[test]
fn test_cloud_credentials_save_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let creds_dir = temp_dir.path().join(".systemprompt");
    let creds_path = creds_dir.join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(token, "https://api.test.io".to_string(), "test@example.com".to_string());

    creds.save_to_path(&creds_path).unwrap();

    let gitignore_path = creds_dir.join(".gitignore");
    assert!(gitignore_path.exists());

    let gitignore_content = std::fs::read_to_string(gitignore_path).unwrap();
    assert_eq!(gitignore_content, "*\n");
}

#[test]
fn test_cloud_credentials_load_from_nonexistent_path() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("nonexistent.json");

    let result = CloudCredentials::load_from_path(&creds_path);
    assert!(result.is_err());
}

#[test]
fn test_cloud_credentials_load_from_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    std::fs::write(&creds_path, "not valid json").unwrap();

    let result = CloudCredentials::load_from_path(&creds_path);
    assert!(result.is_err());
}

#[test]
fn test_cloud_credentials_delete_existing() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    std::fs::write(&creds_path, "{}").unwrap();
    assert!(creds_path.exists());

    CloudCredentials::delete_from_path(&creds_path).unwrap();
    assert!(!creds_path.exists());
}

#[test]
fn test_cloud_credentials_delete_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("nonexistent.json");

    let result = CloudCredentials::delete_from_path(&creds_path);
    assert!(result.is_ok());
}

#[test]
fn test_cloud_credentials_debug() {
    let creds = CloudCredentials::new(
        "secret_token".to_string(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );

    let debug_str = format!("{:?}", creds);
    assert!(debug_str.contains("CloudCredentials"));
    assert!(debug_str.contains("api_url"));
}

#[test]
fn test_cloud_credentials_clone() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        token.clone(),
        "https://api.test.io".to_string(),
        "test@example.com".to_string(),
    );

    let cloned = creds.clone();
    assert_eq!(cloned.api_token, creds.api_token);
    assert_eq!(cloned.api_url, creds.api_url);
    assert_eq!(cloned.user_email, creds.user_email);
}
