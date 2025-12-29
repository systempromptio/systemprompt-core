//! Unit tests for CloudCredentials
//!
//! Tests cover:
//! - CloudCredentials::new creation
//! - is_token_expired functionality
//! - expires_within functionality
//! - Serialization and deserialization
//! - Validation behavior
//! - File operations (save/load/delete) with temp files

use base64::prelude::*;
use chrono::{Duration, Utc};
use std::fs;
use systemprompt_cloud::CloudCredentials;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a valid JWT token with a specific expiry timestamp
fn create_test_token(exp: i64) -> String {
    let header = BASE64_URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = BASE64_URL_SAFE_NO_PAD.encode(format!(r#"{{"exp":{}}}"#, exp));
    let signature = BASE64_URL_SAFE_NO_PAD.encode("test_signature");
    format!("{}.{}.{}", header, payload, signature)
}

fn create_valid_token() -> String {
    create_test_token(Utc::now().timestamp() + 3600) // Expires in 1 hour
}

fn create_expired_token() -> String {
    create_test_token(Utc::now().timestamp() - 3600) // Expired 1 hour ago
}

fn create_expiring_soon_token() -> String {
    create_test_token(Utc::now().timestamp() + 1800) // Expires in 30 minutes
}

// ============================================================================
// CloudCredentials::new Tests
// ============================================================================

#[test]
fn test_credentials_new() {
    let token = create_valid_token();
    let creds = CloudCredentials::new(
        token.clone(),
        "https://api.test.com".to_string(),
        Some("user@example.com".to_string()),
    );

    assert_eq!(creds.api_token, token);
    assert_eq!(creds.api_url, "https://api.test.com");
    assert_eq!(creds.user_email, Some("user@example.com".to_string()));
}

#[test]
fn test_credentials_new_sets_authenticated_at() {
    let before = Utc::now();
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );
    let after = Utc::now();

    assert!(creds.authenticated_at >= before);
    assert!(creds.authenticated_at <= after);
}

#[test]
fn test_credentials_new_without_email() {
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );

    assert!(creds.user_email.is_none());
}

// ============================================================================
// is_token_expired Tests
// ============================================================================

#[test]
fn test_credentials_is_token_expired_false() {
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );

    assert!(!creds.is_token_expired());
}

#[test]
fn test_credentials_is_token_expired_true() {
    let creds = CloudCredentials::new(
        create_expired_token(),
        "https://api.test.com".to_string(),
        None,
    );

    assert!(creds.is_token_expired());
}

// ============================================================================
// expires_within Tests
// ============================================================================

#[test]
fn test_credentials_expires_within_true() {
    let creds = CloudCredentials::new(
        create_expiring_soon_token(), // Expires in 30 min
        "https://api.test.com".to_string(),
        None,
    );

    assert!(creds.expires_within(Duration::hours(1)));
}

#[test]
fn test_credentials_expires_within_false() {
    let creds = CloudCredentials::new(
        create_valid_token(), // Expires in 1 hour
        "https://api.test.com".to_string(),
        None,
    );

    assert!(!creds.expires_within(Duration::minutes(30)));
}

// ============================================================================
// Serialization Tests
// ============================================================================

#[test]
fn test_credentials_serialization() {
    let token = create_valid_token();
    let creds = CloudCredentials::new(
        token.clone(),
        "https://api.test.com".to_string(),
        Some("test@example.com".to_string()),
    );

    let json = serde_json::to_string(&creds).unwrap();
    assert!(json.contains(&token));
    assert!(json.contains("https://api.test.com"));
    assert!(json.contains("test@example.com"));
    assert!(json.contains("authenticated_at"));
}

#[test]
fn test_credentials_serialization_without_email() {
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );

    let json = serde_json::to_string(&creds).unwrap();
    // user_email should be skipped when None
    assert!(!json.contains("user_email"));
}

#[test]
fn test_credentials_deserialization() {
    let token = create_valid_token();
    let json = format!(
        r#"{{
            "api_token": "{}",
            "api_url": "https://api.test.com",
            "authenticated_at": "2024-01-15T12:00:00Z",
            "user_email": "user@test.com"
        }}"#,
        token
    );

    let creds: CloudCredentials = serde_json::from_str(&json).unwrap();
    assert_eq!(creds.api_token, token);
    assert_eq!(creds.api_url, "https://api.test.com");
    assert_eq!(creds.user_email, Some("user@test.com".to_string()));
}

#[test]
fn test_credentials_deserialization_without_optional() {
    let token = create_valid_token();
    let json = format!(
        r#"{{
            "api_token": "{}",
            "api_url": "https://api.test.com",
            "authenticated_at": "2024-01-15T12:00:00Z"
        }}"#,
        token
    );

    let creds: CloudCredentials = serde_json::from_str(&json).unwrap();
    assert!(creds.user_email.is_none());
}

#[test]
fn test_credentials_roundtrip() {
    let original = CloudCredentials::new(
        create_valid_token(),
        "https://api.roundtrip.com".to_string(),
        Some("roundtrip@test.com".to_string()),
    );

    let json = serde_json::to_string(&original).unwrap();
    let restored: CloudCredentials = serde_json::from_str(&json).unwrap();

    assert_eq!(restored.api_token, original.api_token);
    assert_eq!(restored.api_url, original.api_url);
    assert_eq!(restored.user_email, original.user_email);
    assert_eq!(restored.authenticated_at, original.authenticated_at);
}

// ============================================================================
// File Operations Tests
// ============================================================================

#[test]
fn test_credentials_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    let original = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        Some("test@example.com".to_string()),
    );

    // Save
    original.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());

    // Load
    let loaded = CloudCredentials::load_from_path(&creds_path).unwrap();
    assert_eq!(loaded.api_token, original.api_token);
    assert_eq!(loaded.api_url, original.api_url);
    assert_eq!(loaded.user_email, original.user_email);
}

#[test]
fn test_credentials_save_creates_parent_dir() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("subdir").join("credentials.json");

    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );

    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());
    assert!(temp_dir.path().join("subdir").exists());
}

#[test]
fn test_credentials_save_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let subdir = temp_dir.path().join("secrets");
    let creds_path = subdir.join("credentials.json");

    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );

    creds.save_to_path(&creds_path).unwrap();

    let gitignore_path = subdir.join(".gitignore");
    assert!(gitignore_path.exists());

    let content = fs::read_to_string(&gitignore_path).unwrap();
    assert_eq!(content, "*\n");
}

#[test]
fn test_credentials_load_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("nonexistent.json");

    let result = CloudCredentials::load_from_path(&creds_path);
    assert!(result.is_err());
}

#[test]
fn test_credentials_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    fs::write(&creds_path, "not valid json").unwrap();

    let result = CloudCredentials::load_from_path(&creds_path);
    assert!(result.is_err());
}

#[test]
fn test_credentials_delete() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    // Create file first
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        None,
    );
    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());

    // Delete
    CloudCredentials::delete_from_path(&creds_path).unwrap();
    assert!(!creds_path.exists());
}

#[test]
fn test_credentials_delete_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("nonexistent.json");

    // Deleting nonexistent file should not error
    let result = CloudCredentials::delete_from_path(&creds_path);
    assert!(result.is_ok());
}

// ============================================================================
// Debug Trait Tests
// ============================================================================

#[test]
fn test_credentials_debug() {
    let creds = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        Some("debug@test.com".to_string()),
    );

    let debug_str = format!("{:?}", creds);
    assert!(debug_str.contains("CloudCredentials"));
    assert!(debug_str.contains("api_url"));
}

// ============================================================================
// Clone Trait Tests
// ============================================================================

#[test]
fn test_credentials_clone() {
    let original = CloudCredentials::new(
        create_valid_token(),
        "https://api.test.com".to_string(),
        Some("clone@test.com".to_string()),
    );

    let cloned = original.clone();

    assert_eq!(cloned.api_token, original.api_token);
    assert_eq!(cloned.api_url, original.api_url);
    assert_eq!(cloned.user_email, original.user_email);
    assert_eq!(cloned.authenticated_at, original.authenticated_at);
}
