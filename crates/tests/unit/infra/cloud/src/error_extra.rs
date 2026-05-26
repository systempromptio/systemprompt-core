//! Additional CloudError tests covering variants not exercised in
//! `error.rs` (Network/Io/Json conversions, Unauthorized, HttpStatus,
//! ApiError/ApiValidationFailed/InvalidCredentials, Credentials* states,
//! SessionVersionMismatch, OAuthFlow/CheckoutFlow/SseStream/ProvisioningFailed,
//! Other) plus the `is_local_mode_recoverable` predicate.

use systemprompt_cloud::CloudError;

fn make_json_error() -> serde_json::Error {
    serde_json::from_str::<serde_json::Value>("not json").unwrap_err()
}

#[test]
fn test_api_error_display() {
    let err = CloudError::ApiError {
        message: "boom".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("API error"));
    assert!(s.contains("boom"));
    assert_eq!(err.user_message(), "API request failed");
    assert!(err.recovery_hint().contains("Check the error message"));
}

#[test]
fn test_api_validation_failed_display() {
    let err = CloudError::ApiValidationFailed {
        message: "token rejected".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("Cloud API validation failed"));
    assert!(s.contains("token rejected"));
    assert_eq!(err.user_message(), "Cloud API rejected credentials");
    assert!(err.is_local_mode_recoverable());
}

#[test]
fn test_invalid_credentials_display() {
    let err = CloudError::InvalidCredentials {
        message: "bad shape".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("Cloud credentials file invalid"));
    assert!(s.contains("bad shape"));
    assert_eq!(err.user_message(), "Stored credentials are invalid");
    assert!(err.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_credentials_file_not_found_display() {
    let err = CloudError::CredentialsFileNotFound {
        path: "/tmp/missing.json".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("Cloud credentials file not found"));
    assert!(s.contains("/tmp/missing.json"));
    assert!(err.is_missing_credentials_file());
    assert!(err.is_local_mode_recoverable());
    assert_eq!(err.user_message(), "Credentials file is missing");
    assert!(err.recovery_hint().contains("systemprompt cloud login"));
}

#[test]
fn test_credentials_not_initialized() {
    let err = CloudError::CredentialsNotInitialized;
    assert_eq!(err.to_string(), "Credentials not initialized");
    assert_eq!(err.user_message(), "Credentials bootstrap not initialised");
    assert!(err.recovery_hint().contains("Restart the process"));
    assert!(!err.requires_login());
    assert!(!err.requires_setup());
}

#[test]
fn test_credentials_already_initialized() {
    let err = CloudError::CredentialsAlreadyInitialized;
    assert_eq!(err.to_string(), "Credentials already initialized");
    assert_eq!(err.user_message(), "Credentials bootstrap already initialised");
}

#[test]
fn test_session_version_mismatch_display() {
    let err = CloudError::SessionVersionMismatch {
        min: 1,
        max: 3,
        actual: 0,
        path: "/tmp/session.json".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("Session file version mismatch"));
    assert!(s.contains("/tmp/session.json"));
    assert_eq!(err.user_message(), "CLI session file is out of date");
    assert!(err.recovery_hint().contains("Delete the session file"));
}

#[test]
fn test_oauth_flow() {
    let err = CloudError::OAuthFlow {
        message: "state mismatch".to_string(),
    };
    assert!(err.to_string().contains("state mismatch"));
    assert_eq!(err.user_message(), "OAuth login flow failed");
    assert!(err.recovery_hint().contains("OAuth"));
}

#[test]
fn test_checkout_flow() {
    let err = CloudError::CheckoutFlow {
        message: "timeout".to_string(),
    };
    assert!(err.to_string().contains("timeout"));
    assert_eq!(err.user_message(), "Cloud checkout flow failed");
    assert!(err.recovery_hint().contains("checkout"));
}

#[test]
fn test_sse_stream() {
    let err = CloudError::SseStream {
        message: "connection reset".to_string(),
    };
    assert!(err.to_string().contains("connection reset"));
    assert_eq!(err.user_message(), "Cloud SSE stream failed");
    assert!(err.recovery_hint().contains("polling"));
}

#[test]
fn test_provisioning_failed() {
    let err = CloudError::ProvisioningFailed {
        message: "db unavailable".to_string(),
    };
    assert!(err.to_string().contains("db unavailable"));
    assert_eq!(err.user_message(), "Tenant provisioning failed");
    assert!(err.recovery_hint().contains("status"));
}

#[test]
fn test_unauthorized() {
    let err = CloudError::Unauthorized;
    assert!(err.to_string().contains("Authentication failed"));
    assert_eq!(err.user_message(), "Cloud API rejected this token");
    assert!(err.requires_login());
}

#[test]
fn test_http_status() {
    let err = CloudError::HttpStatus {
        status: 502,
        body: "bad gateway".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("502"));
    assert!(s.contains("bad gateway"));
    assert_eq!(err.user_message(), "Cloud API returned a non-success status");
}

#[test]
fn test_other_via_helper() {
    let err = CloudError::other("misc");
    assert_eq!(err.to_string(), "misc");
    assert_eq!(err.user_message(), "Cloud operation failed");
    assert!(err.recovery_hint().contains("error message"));
}

#[test]
fn test_io_from_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "nope");
    let err: CloudError = io_err.into();
    assert!(matches!(err, CloudError::Io(_)));
    assert_eq!(err.user_message(), "File system error");
    assert!(err.recovery_hint().contains("permissions"));
}

#[test]
fn test_json_from_conversion() {
    let err: CloudError = make_json_error().into();
    assert!(matches!(err, CloudError::Json(_)));
    assert_eq!(err.user_message(), "JSON parse error");
    assert!(err.recovery_hint().contains("JSON file"));
}

#[test]
fn test_requires_login_false_for_misc() {
    assert!(!CloudError::AppNotConfigured.requires_login());
    assert!(!CloudError::CredentialsAlreadyInitialized.requires_login());
    assert!(!CloudError::HttpStatus { status: 500, body: String::new() }.requires_login());
}

#[test]
fn test_requires_setup_false_for_misc() {
    assert!(!CloudError::Unauthorized.requires_setup());
    assert!(!CloudError::OAuthFlow { message: "x".to_string() }.requires_setup());
}

#[test]
fn test_is_local_mode_recoverable_token_expired() {
    assert!(CloudError::TokenExpired.is_local_mode_recoverable());
}

#[test]
fn test_is_local_mode_recoverable_negative() {
    assert!(!CloudError::Unauthorized.is_local_mode_recoverable());
    assert!(!CloudError::NotAuthenticated.is_local_mode_recoverable());
}

#[test]
fn test_all_variants_have_user_message() {
    let variants = vec![
        CloudError::NotAuthenticated,
        CloudError::TokenExpired,
        CloudError::TenantNotConfigured,
        CloudError::AppNotConfigured,
        CloudError::JwtDecode,
        CloudError::TenantsNotSynced,
        CloudError::CredentialsNotInitialized,
        CloudError::CredentialsAlreadyInitialized,
        CloudError::Unauthorized,
    ];
    for v in variants {
        assert!(!v.user_message().is_empty());
        assert!(!v.recovery_hint().is_empty());
    }
}
