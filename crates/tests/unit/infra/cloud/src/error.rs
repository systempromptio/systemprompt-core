//! Unit tests for CloudError types
//!
//! Tests cover:
//! - CloudError variant creation and display messages
//! - Helper methods (missing_cargo_target, missing_web_dist, missing_dockerfile)
//! - user_message() for all variants
//! - recovery_hint() for all variants
//! - requires_login() predicate
//! - requires_setup() predicate

use systemprompt_cloud::CloudError;

// ============================================================================
// CloudError Display Messages Tests
// ============================================================================

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
fn test_tenant_not_configured_display() {
    let error = CloudError::TenantNotConfigured;
    let msg = error.to_string();
    assert!(msg.contains("No tenant configured"));
    assert!(msg.contains("systemprompt cloud setup"));
}

#[test]
fn test_app_not_configured_display() {
    let error = CloudError::AppNotConfigured;
    let msg = error.to_string();
    assert!(msg.contains("No app configured"));
    assert!(msg.contains("systemprompt cloud setup"));
}

#[test]
fn test_cloud_disabled_display() {
    let error = CloudError::CloudDisabled;
    let msg = error.to_string();
    assert!(msg.contains("Cloud features are disabled"));
    assert!(msg.contains("cloud.cli_enabled: true"));
}

#[test]
fn test_profile_required_display() {
    let error = CloudError::ProfileRequired {
        message: "Profile not found".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("Profile required"));
    assert!(msg.contains("Profile not found"));
    assert!(msg.contains("SYSTEMPROMPT_PROFILE"));
}

#[test]
fn test_missing_profile_field_display() {
    let error = CloudError::MissingProfileField {
        field: "paths.cargo_target".to_string(),
        example: "paths:\n  cargo_target: /path/to/target".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("Missing profile field"));
    assert!(msg.contains("paths.cargo_target"));
    assert!(msg.contains("Add to your profile"));
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
fn test_tenant_not_found_display() {
    let error = CloudError::TenantNotFound {
        tenant_id: "tenant-123".to_string(),
    };
    let msg = error.to_string();
    assert!(msg.contains("Tenant 'tenant-123' not found"));
    assert!(msg.contains("systemprompt cloud config"));
}

// ============================================================================
// user_message() Tests
// ============================================================================

#[test]
fn test_user_message_not_authenticated() {
    let error = CloudError::NotAuthenticated;
    assert_eq!(error.user_message(), "Not logged in to SystemPrompt Cloud");
}

#[test]
fn test_user_message_token_expired() {
    let error = CloudError::TokenExpired;
    assert_eq!(error.user_message(), "Your session has expired");
}

#[test]
fn test_user_message_tenant_not_configured() {
    let error = CloudError::TenantNotConfigured;
    assert_eq!(
        error.user_message(),
        "No project linked to this environment"
    );
}

#[test]
fn test_user_message_app_not_configured() {
    let error = CloudError::AppNotConfigured;
    assert_eq!(error.user_message(), "No deployment target configured");
}

#[test]
fn test_user_message_cloud_disabled() {
    let error = CloudError::CloudDisabled;
    assert_eq!(
        error.user_message(),
        "Cloud features are disabled in this profile"
    );
}

#[test]
fn test_user_message_profile_required() {
    let error = CloudError::ProfileRequired {
        message: "test".to_string(),
    };
    assert_eq!(error.user_message(), "Profile configuration required");
}

#[test]
fn test_user_message_missing_profile_field() {
    let error = CloudError::MissingProfileField {
        field: "test".to_string(),
        example: "test".to_string(),
    };
    assert_eq!(error.user_message(), "Missing required profile field");
}

#[test]
fn test_user_message_jwt_decode() {
    let error = CloudError::JwtDecode;
    assert_eq!(error.user_message(), "Failed to decode authentication token");
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
fn test_user_message_tenant_not_found() {
    let error = CloudError::TenantNotFound {
        tenant_id: "test".to_string(),
    };
    assert_eq!(error.user_message(), "Tenant not found");
}

// ============================================================================
// recovery_hint() Tests
// ============================================================================

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
fn test_recovery_hint_tenant_not_configured() {
    let error = CloudError::TenantNotConfigured;
    assert!(error.recovery_hint().contains("systemprompt cloud setup"));
}

#[test]
fn test_recovery_hint_app_not_configured() {
    let error = CloudError::AppNotConfigured;
    assert!(error.recovery_hint().contains("systemprompt cloud setup"));
}

#[test]
fn test_recovery_hint_cloud_disabled() {
    let error = CloudError::CloudDisabled;
    assert!(error.recovery_hint().contains("cloud.cli_enabled: true"));
}

#[test]
fn test_recovery_hint_profile_required() {
    let error = CloudError::ProfileRequired {
        message: "test".to_string(),
    };
    assert!(error.recovery_hint().contains("SYSTEMPROMPT_PROFILE"));
}

#[test]
fn test_recovery_hint_missing_profile_field() {
    let error = CloudError::MissingProfileField {
        field: "test".to_string(),
        example: "test".to_string(),
    };
    assert!(error.recovery_hint().contains("profile YAML"));
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
fn test_recovery_hint_tenant_not_found() {
    let error = CloudError::TenantNotFound {
        tenant_id: "test".to_string(),
    };
    assert!(error.recovery_hint().contains("systemprompt cloud config"));
}

// ============================================================================
// requires_login() Tests
// ============================================================================

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
fn test_requires_login_false_for_tenant_not_configured() {
    let error = CloudError::TenantNotConfigured;
    assert!(!error.requires_login());
}

#[test]
fn test_requires_login_false_for_cloud_disabled() {
    let error = CloudError::CloudDisabled;
    assert!(!error.requires_login());
}

#[test]
fn test_requires_login_false_for_jwt_decode() {
    let error = CloudError::JwtDecode;
    assert!(!error.requires_login());
}

// ============================================================================
// requires_setup() Tests
// ============================================================================

#[test]
fn test_requires_setup_true_for_tenant_not_configured() {
    let error = CloudError::TenantNotConfigured;
    assert!(error.requires_setup());
}

#[test]
fn test_requires_setup_true_for_app_not_configured() {
    let error = CloudError::AppNotConfigured;
    assert!(error.requires_setup());
}

#[test]
fn test_requires_setup_false_for_not_authenticated() {
    let error = CloudError::NotAuthenticated;
    assert!(!error.requires_setup());
}

#[test]
fn test_requires_setup_false_for_token_expired() {
    let error = CloudError::TokenExpired;
    assert!(!error.requires_setup());
}

#[test]
fn test_requires_setup_false_for_cloud_disabled() {
    let error = CloudError::CloudDisabled;
    assert!(!error.requires_setup());
}

// ============================================================================
// Error Debug Trait Tests
// ============================================================================

#[test]
fn test_cloud_error_debug() {
    let error = CloudError::NotAuthenticated;
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("NotAuthenticated"));
}

#[test]
fn test_cloud_error_debug_with_fields() {
    let error = CloudError::TenantNotFound {
        tenant_id: "test-123".to_string(),
    };
    let debug_str = format!("{:?}", error);
    assert!(debug_str.contains("TenantNotFound"));
    assert!(debug_str.contains("test-123"));
}
