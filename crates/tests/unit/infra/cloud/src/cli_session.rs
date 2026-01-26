//! Unit tests for CliSession and CliSessionBuilder

use chrono::{Duration, Utc};
use std::path::PathBuf;
use systemprompt_cloud::cli_session::{CliSession, CliSessionBuilder, SessionKey, LOCAL_SESSION_KEY};
use systemprompt_identifiers::{ContextId, Email, ProfileName, SessionId, SessionToken, TenantId, UserId};
use systemprompt_models::auth::UserType;
use tempfile::TempDir;

fn create_test_builder() -> CliSessionBuilder {
    CliSessionBuilder::new(
        ProfileName::new("test-profile"),
        SessionToken::new("test-token"),
        SessionId::new("session-123"),
        ContextId::new("context-456"),
    )
}

#[test]
fn test_cli_session_builder_new() {
    let builder = create_test_builder();
    let session = builder.build();

    assert_eq!(session.profile_name.as_str(), "test-profile");
    assert_eq!(session.session_token.as_str(), "test-token");
    assert_eq!(session.session_id.as_str(), "session-123");
    assert_eq!(session.context_id.as_str(), "context-456");
}

#[test]
fn test_cli_session_builder_default_user() {
    let builder = create_test_builder();
    let session = builder.build();

    assert_eq!(session.user_id, UserId::system());
    assert_eq!(session.user_email.as_str(), "system@local.invalid");
}

#[test]
fn test_cli_session_builder_default_user_type() {
    let builder = create_test_builder();
    let session = builder.build();

    assert_eq!(session.user_type, UserType::Admin);
}

#[test]
fn test_cli_session_builder_with_tenant_key() {
    let builder = create_test_builder()
        .with_tenant_key(TenantId::new("my-tenant"));
    let session = builder.build();

    assert_eq!(session.tenant_key, Some(TenantId::new("my-tenant")));
}

#[test]
fn test_cli_session_builder_with_session_key_local() {
    let builder = create_test_builder()
        .with_session_key(&SessionKey::Local);
    let session = builder.build();

    assert_eq!(session.tenant_key, Some(TenantId::new(LOCAL_SESSION_KEY)));
}

#[test]
fn test_cli_session_builder_with_session_key_tenant() {
    let tenant_id = TenantId::new("specific-tenant");
    let builder = create_test_builder()
        .with_session_key(&SessionKey::Tenant(tenant_id.clone()));
    let session = builder.build();

    assert_eq!(session.tenant_key, Some(tenant_id));
}

#[test]
fn test_cli_session_builder_with_profile_path() {
    let builder = create_test_builder()
        .with_profile_path("/path/to/profile.yaml");
    let session = builder.build();

    assert_eq!(session.profile_path, Some(PathBuf::from("/path/to/profile.yaml")));
}

#[test]
fn test_cli_session_builder_with_user() {
    let user_id = UserId::new("user-789");
    let email = Email::new("test@example.com");
    let builder = create_test_builder()
        .with_user(user_id.clone(), email.clone());
    let session = builder.build();

    assert_eq!(session.user_id, user_id);
    assert_eq!(session.user_email, email);
}

#[test]
fn test_cli_session_builder_with_user_type() {
    let builder = create_test_builder()
        .with_user_type(UserType::User);
    let session = builder.build();

    assert_eq!(session.user_type, UserType::User);
}

#[test]
fn test_cli_session_builder_chain() {
    let session = create_test_builder()
        .with_tenant_key(TenantId::new("tenant"))
        .with_profile_path("/profile.yaml")
        .with_user(UserId::new("user"), Email::new("u@e.com"))
        .with_user_type(UserType::User)
        .build();

    assert_eq!(session.tenant_key, Some(TenantId::new("tenant")));
    assert_eq!(session.profile_path, Some(PathBuf::from("/profile.yaml")));
    assert_eq!(session.user_id, UserId::new("user"));
    assert_eq!(session.user_type, UserType::User);
}

#[test]
fn test_cli_session_timestamps() {
    let before = Utc::now();
    let session = create_test_builder().build();
    let after = Utc::now();

    assert!(session.created_at >= before);
    assert!(session.created_at <= after);
    assert!(session.last_used >= before);
    assert!(session.last_used <= after);
}

#[test]
fn test_cli_session_expires_at() {
    let session = create_test_builder().build();
    let expected_expiry = session.created_at + Duration::hours(24);

    assert_eq!(session.expires_at, expected_expiry);
}

#[test]
fn test_cli_session_context_id() {
    let session = create_test_builder().build();
    assert_eq!(session.context_id().as_str(), "context-456");
}

#[test]
fn test_cli_session_touch() {
    let mut session = create_test_builder().build();
    let original_last_used = session.last_used;

    std::thread::sleep(std::time::Duration::from_millis(10));
    session.touch();

    assert!(session.last_used > original_last_used);
}

#[test]
fn test_cli_session_set_context_id() {
    let mut session = create_test_builder().build();
    let original_last_used = session.last_used;

    std::thread::sleep(std::time::Duration::from_millis(10));
    session.set_context_id(ContextId::new("new-context"));

    assert_eq!(session.context_id.as_str(), "new-context");
    assert!(session.last_used > original_last_used);
}

#[test]
fn test_cli_session_is_expired_false_for_fresh() {
    let session = create_test_builder().build();
    assert!(!session.is_expired());
}

#[test]
fn test_cli_session_is_expired_true_for_past_expiry() {
    let mut session = create_test_builder().build();
    session.expires_at = Utc::now() - Duration::hours(1);
    assert!(session.is_expired());
}

#[test]
fn test_cli_session_is_valid_for_profile_true() {
    let session = create_test_builder().build();
    assert!(session.is_valid_for_profile("test-profile"));
}

#[test]
fn test_cli_session_is_valid_for_profile_false_wrong_profile() {
    let session = create_test_builder().build();
    assert!(!session.is_valid_for_profile("other-profile"));
}

#[test]
fn test_cli_session_is_valid_for_profile_false_expired() {
    let mut session = create_test_builder().build();
    session.expires_at = Utc::now() - Duration::hours(1);
    assert!(!session.is_valid_for_profile("test-profile"));
}

#[test]
fn test_cli_session_has_valid_credentials_true() {
    let session = create_test_builder().build();
    assert!(session.has_valid_credentials());
}

#[test]
fn test_cli_session_has_valid_credentials_false_empty_token() {
    let session = CliSessionBuilder::new(
        ProfileName::new("profile"),
        SessionToken::new(""),
        SessionId::new("session"),
        ContextId::new("context"),
    ).build();

    assert!(!session.has_valid_credentials());
}

#[test]
fn test_cli_session_is_valid_for_tenant_local_none() {
    let session = create_test_builder().build();
    assert!(session.is_valid_for_tenant(&SessionKey::Local));
}

#[test]
fn test_cli_session_is_valid_for_tenant_local_key() {
    let session = create_test_builder()
        .with_session_key(&SessionKey::Local)
        .build();
    assert!(session.is_valid_for_tenant(&SessionKey::Local));
}

#[test]
fn test_cli_session_is_valid_for_tenant_matching_tenant() {
    let tenant_id = TenantId::new("my-tenant");
    let session = create_test_builder()
        .with_tenant_key(tenant_id.clone())
        .build();
    assert!(session.is_valid_for_tenant(&SessionKey::Tenant(tenant_id)));
}

#[test]
fn test_cli_session_is_valid_for_tenant_wrong_tenant() {
    let session = create_test_builder()
        .with_tenant_key(TenantId::new("tenant-a"))
        .build();
    assert!(!session.is_valid_for_tenant(&SessionKey::Tenant(TenantId::new("tenant-b"))));
}

#[test]
fn test_cli_session_is_valid_for_tenant_expired() {
    let mut session = create_test_builder()
        .with_session_key(&SessionKey::Local)
        .build();
    session.expires_at = Utc::now() - Duration::hours(1);
    assert!(!session.is_valid_for_tenant(&SessionKey::Local));
}

#[test]
fn test_cli_session_session_key_local_none() {
    let session = create_test_builder().build();
    assert!(matches!(session.session_key(), SessionKey::Local));
}

#[test]
fn test_cli_session_session_key_local_explicit() {
    let session = create_test_builder()
        .with_session_key(&SessionKey::Local)
        .build();
    assert!(matches!(session.session_key(), SessionKey::Local));
}

#[test]
fn test_cli_session_session_key_tenant() {
    let session = create_test_builder()
        .with_tenant_key(TenantId::new("my-tenant"))
        .build();
    assert!(matches!(session.session_key(), SessionKey::Tenant(_)));
}

#[test]
fn test_cli_session_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("session.json");

    let session = create_test_builder()
        .with_tenant_key(TenantId::new("test-tenant"))
        .with_user(UserId::new("user-1"), Email::new("test@test.com"))
        .build();

    session.save_to_path(&session_path).unwrap();
    assert!(session_path.exists());

    let loaded = CliSession::load_from_path(&session_path).unwrap();
    assert_eq!(loaded.profile_name.as_str(), session.profile_name.as_str());
    assert_eq!(loaded.session_token.as_str(), session.session_token.as_str());
    assert_eq!(loaded.user_id, session.user_id);
}

#[test]
fn test_cli_session_save_creates_parent_dirs() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("nested").join("dir").join("session.json");

    let session = create_test_builder().build();
    session.save_to_path(&session_path).unwrap();

    assert!(session_path.exists());
}

#[test]
fn test_cli_session_save_creates_gitignore() {
    let temp_dir = TempDir::new().unwrap();
    let session_dir = temp_dir.path().join(".systemprompt");
    let session_path = session_dir.join("session.json");

    let session = create_test_builder().build();
    session.save_to_path(&session_path).unwrap();

    let gitignore_path = session_dir.join(".gitignore");
    assert!(gitignore_path.exists());
}

#[test]
fn test_cli_session_load_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("nonexistent.json");

    let result = CliSession::load_from_path(&session_path);
    assert!(result.is_err());
}

#[test]
fn test_cli_session_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("session.json");

    std::fs::write(&session_path, "not valid json").unwrap();

    let result = CliSession::load_from_path(&session_path);
    assert!(result.is_err());
}

#[test]
fn test_cli_session_delete_existing() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("session.json");

    std::fs::write(&session_path, "{}").unwrap();
    assert!(session_path.exists());

    CliSession::delete_from_path(&session_path).unwrap();
    assert!(!session_path.exists());
}

#[test]
fn test_cli_session_delete_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("nonexistent.json");

    let result = CliSession::delete_from_path(&session_path);
    assert!(result.is_ok());
}

#[test]
fn test_cli_session_load_from_path_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("nonexistent.json");

    let result = CliSession::load_from_path(&session_path);
    assert!(result.is_err());
}

#[test]
fn test_cli_session_load_from_path_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let session_path = temp_dir.path().join("session.json");

    std::fs::write(&session_path, "invalid json").unwrap();

    let result = CliSession::load_from_path(&session_path);
    assert!(result.is_err());
}

#[test]
fn test_cli_session_builder_method() {
    let builder = CliSession::builder(
        ProfileName::new("p"),
        SessionToken::new("t"),
        SessionId::new("s"),
        ContextId::new("c"),
    );
    let session = builder.build();

    assert_eq!(session.profile_name.as_str(), "p");
}

#[test]
fn test_cli_session_serialization() {
    let session = create_test_builder().build();
    let json = serde_json::to_string(&session).unwrap();

    assert!(json.contains("test-profile"));
    assert!(json.contains("session-123"));
    assert!(json.contains("version"));
}

#[test]
fn test_cli_session_clone() {
    let session = create_test_builder().build();
    let cloned = session.clone();

    assert_eq!(session.profile_name.as_str(), cloned.profile_name.as_str());
    assert_eq!(session.session_id.as_str(), cloned.session_id.as_str());
}

#[test]
fn test_cli_session_debug() {
    let session = create_test_builder().build();
    let debug_str = format!("{:?}", session);

    assert!(debug_str.contains("CliSession"));
    assert!(debug_str.contains("profile_name"));
}
