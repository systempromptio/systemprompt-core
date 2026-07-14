//! Unit tests for CloudError types
//!
//! Tests cover:
//! - CloudError variant creation and display messages
//! - user_message() for all variants
//! - recovery_hint() for all variants
//! - requires_login() predicate

use systemprompt_cloud::CloudError;

#[test]
fn test_not_authenticated_display() {
    let error = CloudError::NotAuthenticated;
    let msg = error.to_string();
    assert!(msg.contains("Authentication required"));
    assert!(msg.contains("systemprompt cloud login"));
}

#[test]
fn test_token_expired_display() {
    let error = CloudError::TokenExpired;
    let msg = error.to_string();
    assert!(msg.contains("Token expired"));
    assert!(msg.contains("systemprompt cloud login"));
}

#[test]
fn test_jwt_decode_display() {
    let error = CloudError::JwtDecode;
    assert_eq!(error.to_string(), "JWT decode error");
}

#[test]
fn test_credentials_corrupted_display() {
    let json_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
    let error = CloudError::CredentialsCorrupted { source: json_error };
    let msg = error.to_string();
    assert!(msg.contains("Credentials file corrupted"));
}

#[test]
fn test_tenants_not_synced_display() {
    let error = CloudError::TenantsNotSynced;
    let msg = error.to_string();
    assert!(msg.contains("Tenants not synced"));
    assert!(msg.contains("systemprompt cloud login"));
}

#[test]
fn test_tenants_store_corrupted_display() {
    let json_error = serde_json::from_str::<serde_json::Value>("{invalid json").unwrap_err();
    let error = CloudError::TenantsStoreCorrupted { source: json_error };
    let msg = error.to_string();
    assert!(msg.contains("Tenants store corrupted"));
}

#[test]
fn test_tenants_store_invalid_display() {
    let error = CloudError::TenantsStoreInvalid {
        message: "Missing required field".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("Tenants store invalid"));
    assert!(msg.contains("Missing required field"));
}

#[test]
fn test_user_message_not_authenticated() {
    let error = CloudError::NotAuthenticated;
    assert_eq!(
        error.user_message(),
        "Not logged in to systemprompt.io Cloud"
    );
}

#[test]
fn test_user_message_token_expired() {
    let error = CloudError::TokenExpired;
    assert_eq!(error.user_message(), "Your session has expired");
}

#[test]
fn test_user_message_jwt_decode() {
    let error = CloudError::JwtDecode;
    assert_eq!(
        error.user_message(),
        "Failed to decode authentication token"
    );
}

#[test]
fn test_user_message_credentials_corrupted() {
    let json_error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error = CloudError::CredentialsCorrupted { source: json_error };
    assert_eq!(error.user_message(), "Credentials file is corrupted");
}

#[test]
fn test_user_message_tenants_not_synced() {
    let error = CloudError::TenantsNotSynced;
    assert_eq!(error.user_message(), "Tenants not synced locally");
}

#[test]
fn test_user_message_tenants_store_corrupted() {
    let json_error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error = CloudError::TenantsStoreCorrupted { source: json_error };
    assert_eq!(error.user_message(), "Tenants store is corrupted");
}

#[test]
fn test_user_message_tenants_store_invalid() {
    let error = CloudError::TenantsStoreInvalid {
        message: "test".to_string(),
    };
    assert_eq!(error.user_message(), "Tenants store is invalid");
}

#[test]
fn test_recovery_hint_not_authenticated() {
    let error = CloudError::NotAuthenticated;
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_token_expired() {
    let error = CloudError::TokenExpired;
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_jwt_decode() {
    let error = CloudError::JwtDecode;
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_credentials_corrupted() {
    let json_error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error = CloudError::CredentialsCorrupted { source: json_error };
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_tenants_not_synced() {
    let error = CloudError::TenantsNotSynced;
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_tenants_store_corrupted() {
    let json_error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error = CloudError::TenantsStoreCorrupted { source: json_error };
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_recovery_hint_tenants_store_invalid() {
    let error = CloudError::TenantsStoreInvalid {
        message: "test".to_string(),
    };
    assert!(error.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_requires_login_true_for_not_authenticated() {
    let error = CloudError::NotAuthenticated;
    assert!(error.requires_login());
}

#[test]
fn test_requires_login_true_for_token_expired() {
    let error = CloudError::TokenExpired;
    assert!(error.requires_login());
}

#[test]
fn test_requires_login_true_for_credentials_corrupted() {
    let json_error = serde_json::from_str::<serde_json::Value>("bad").unwrap_err();
    let error = CloudError::CredentialsCorrupted { source: json_error };
    assert!(error.requires_login());
}

#[test]
fn test_requires_login_false_for_jwt_decode() {
    let error = CloudError::JwtDecode;
    assert!(!error.requires_login());
}

#[test]
fn is_missing_credentials_file_matches_variant() {
    let error = CloudError::CredentialsFileNotFound {
        path: "/tmp/missing.json".to_string(),
    };
    assert!(error.is_missing_credentials_file());
}

#[test]
fn is_missing_credentials_file_rejects_other_variants() {
    assert!(!CloudError::NotAuthenticated.is_missing_credentials_file());
    assert!(!CloudError::TokenExpired.is_missing_credentials_file());
    assert!(
        !CloudError::InvalidCredentials {
            message: "bad".to_string()
        }
        .is_missing_credentials_file()
    );
}

#[test]
fn test_cloud_error_debug() {
    let error = CloudError::NotAuthenticated;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("NotAuthenticated"));
}
