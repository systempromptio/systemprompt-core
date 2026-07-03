//! Unit tests for CloudCredentials

use base64::prelude::*;
use chrono::{Duration, Utc};
use systemprompt_cloud::CloudCredentials;
use systemprompt_identifiers::{CloudAuthToken, Email};
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
        CloudAuthToken::new("test_token".to_string()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    assert_eq!(creds.api_token.as_str(), "test_token");
    assert_eq!(creds.api_url, "https://api.test.io");
    assert_eq!(creds.user_email.as_str(), "test@example.com");
}

#[test]
fn test_cloud_credentials_authenticated_at_is_now() {
    let before = Utc::now();
    let creds = CloudCredentials::new(
        CloudAuthToken::new("token".to_string()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );
    let after = Utc::now();

    assert!(creds.authenticated_at >= before);
    assert!(creds.authenticated_at <= after);
}

#[test]
fn test_cloud_credentials_token() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token.clone()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    let cloud_token = creds.token();
    assert_eq!(cloud_token.as_str(), &token);
}

#[test]
fn test_cloud_credentials_is_token_expired_false_for_valid() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    assert!(!creds.is_token_expired());
}

#[test]
fn test_cloud_credentials_is_token_expired_true_for_expired() {
    let token = create_valid_token(-3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    assert!(creds.is_token_expired());
}

#[test]
fn test_cloud_credentials_expires_within_true_when_expiring_soon() {
    let token = create_valid_token(1800);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    assert!(creds.expires_within(Duration::hours(1)));
}

#[test]
fn test_cloud_credentials_expires_within_false_when_not_expiring_soon() {
    let token = create_valid_token(7200);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    assert!(!creds.expires_within(Duration::hours(1)));
}

#[test]
fn test_cloud_credentials_serialization() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token.clone()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
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
        CloudAuthToken::new("token".to_string()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    let json = serde_json::to_string(&creds).unwrap();
    assert!(json.contains("user_email"));
    assert!(json.contains("test@example.com"));
}

#[test]
fn test_cloud_credentials_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token.clone()),
        "https://api.systemprompt.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());

    let loaded = CloudCredentials::load_from_path(&creds_path).unwrap();
    assert_eq!(loaded.api_token.as_str(), token);
    assert_eq!(loaded.api_url, "https://api.systemprompt.io");
}

#[test]
fn test_cloud_credentials_save_creates_parent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir
        .path()
        .join("nested")
        .join("dir")
        .join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    creds.save_to_path(&creds_path).unwrap();
    assert!(creds_path.exists());
}

#[test]
fn test_cloud_credentials_save_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let creds_dir = temp_dir.path().join(".systemprompt");
    let creds_path = creds_dir.join("credentials.json");

    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

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
    result.unwrap_err();
}

#[test]
fn test_cloud_credentials_load_from_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");

    std::fs::write(&creds_path, "not valid json").unwrap();

    let result = CloudCredentials::load_from_path(&creds_path);
    result.unwrap_err();
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
    result.expect("result should succeed");
}

#[test]
fn test_load_and_validate_from_path_ok_for_valid() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let token = create_valid_token(7200);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token.clone()),
        "https://api.systemprompt.io".to_string(),
        Email::new("test@example.com".to_string()),
    );
    creds.save_to_path(&creds_path).unwrap();

    let loaded = CloudCredentials::load_and_validate_from_path(&creds_path).unwrap();
    assert_eq!(loaded.api_token.as_str(), token);
}

#[test]
fn test_load_and_validate_from_path_warns_when_expiring_soon_but_ok() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let creds = CloudCredentials::new(
        CloudAuthToken::new(create_valid_token(1800)),
        "https://api.systemprompt.io".to_string(),
        Email::new("test@example.com".to_string()),
    );
    creds.save_to_path(&creds_path).unwrap();

    let loaded = CloudCredentials::load_and_validate_from_path(&creds_path).unwrap();
    assert!(loaded.expires_within(chrono::Duration::hours(1)));
}

#[test]
fn test_load_and_validate_from_path_rejects_expired_token() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let creds = CloudCredentials::new(
        CloudAuthToken::new(create_valid_token(-3600)),
        "https://api.systemprompt.io".to_string(),
        Email::new("test@example.com".to_string()),
    );
    creds.save_to_path(&creds_path).unwrap();

    CloudCredentials::load_and_validate_from_path(&creds_path).unwrap_err();
}

#[test]
fn test_load_and_validate_from_path_rejects_invalid_url() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let raw = serde_json::json!({
        "api_token": create_valid_token(7200),
        "api_url": "not-a-valid-url",
        "authenticated_at": Utc::now().to_rfc3339(),
        "user_email": "test@example.com",
        "last_validated_at": null,
    });
    std::fs::write(&creds_path, raw.to_string()).unwrap();

    CloudCredentials::load_and_validate_from_path(&creds_path).unwrap_err();
}

#[test]
fn test_load_from_path_rejects_invalid_url() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let raw = serde_json::json!({
        "api_token": "tok",
        "api_url": "still-not-a-url",
        "authenticated_at": Utc::now().to_rfc3339(),
        "user_email": "test@example.com",
    });
    std::fs::write(&creds_path, raw.to_string()).unwrap();

    CloudCredentials::load_from_path(&creds_path).unwrap_err();
}

#[test]
fn test_save_to_path_rejects_invalid_url() {
    let temp_dir = TempDir::new().unwrap();
    let creds_path = temp_dir.path().join("credentials.json");
    let creds = CloudCredentials::new(
        CloudAuthToken::new("tok".to_string()),
        "not-a-url".to_string(),
        Email::new("test@example.com".to_string()),
    );

    creds.save_to_path(&creds_path).unwrap_err();
    assert!(!creds_path.exists());
}

#[tokio::test]
async fn test_validate_with_api_true_on_success() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .and(header("authorization", "Bearer tok-abc"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let creds = CloudCredentials::new(
        CloudAuthToken::new("tok-abc".to_string()),
        server.uri(),
        Email::new("test@example.com".to_string()),
    );

    assert!(creds.validate_with_api().await.unwrap());
}

#[tokio::test]
async fn test_validate_with_api_false_on_unauthorized() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/auth/me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let creds = CloudCredentials::new(
        CloudAuthToken::new("bad-tok".to_string()),
        server.uri(),
        Email::new("test@example.com".to_string()),
    );

    assert!(!creds.validate_with_api().await.unwrap());
}

#[test]
fn test_cloud_credentials_debug() {
    let creds = CloudCredentials::new(
        CloudAuthToken::new("secret_token".to_string()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    let debug_str = format!("{:?}", creds);
    assert!(debug_str.contains("CloudCredentials"));
    assert!(debug_str.contains("api_url"));
}

#[test]
fn test_cloud_credentials_clone() {
    let token = create_valid_token(3600);
    let creds = CloudCredentials::new(
        CloudAuthToken::new(token.clone()),
        "https://api.test.io".to_string(),
        Email::new("test@example.com".to_string()),
    );

    let cloned = creds.clone();
    assert_eq!(cloned.api_token, creds.api_token);
    assert_eq!(cloned.api_url, creds.api_url);
    assert_eq!(cloned.user_email, creds.user_email);
}
