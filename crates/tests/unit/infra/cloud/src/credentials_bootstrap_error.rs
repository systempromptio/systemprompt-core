//! Unit tests for `CredentialsBootstrapError` variants, display messages,
//! helper predicates, and conversion into `CloudError`.

use systemprompt_cloud::{CloudError, CredentialsBootstrapError};

#[test]
fn not_initialized_display() {
    let err = CredentialsBootstrapError::NotInitialized;
    assert!(err.to_string().contains("not initialized"));
}

#[test]
fn already_initialized_display() {
    let err = CredentialsBootstrapError::AlreadyInitialized;
    assert!(err.to_string().contains("already initialized"));
}

#[test]
fn not_available_display() {
    let err = CredentialsBootstrapError::NotAvailable;
    assert!(err.to_string().contains("not available"));
}

#[test]
fn file_not_found_display_includes_path() {
    let err = CredentialsBootstrapError::FileNotFound {
        path: "/home/user/.systemprompt/credentials.json".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("not found"));
    assert!(s.contains("/home/user/.systemprompt/credentials.json"));
}

#[test]
fn invalid_credentials_display_includes_message() {
    let err = CredentialsBootstrapError::InvalidCredentials {
        message: "token field missing".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("invalid"));
    assert!(s.contains("token field missing"));
}

#[test]
fn token_expired_display() {
    let err = CredentialsBootstrapError::TokenExpired;
    let s = err.to_string();
    assert!(s.contains("expired"));
}

#[test]
fn api_validation_failed_display_includes_message() {
    let err = CredentialsBootstrapError::ApiValidationFailed {
        message: "401 Unauthorized".to_string(),
    };
    let s = err.to_string();
    assert!(s.contains("validation failed"));
    assert!(s.contains("401 Unauthorized"));
}

#[test]
fn is_file_not_found_true_for_file_not_found() {
    let err = CredentialsBootstrapError::FileNotFound {
        path: "/tmp/x.json".to_string(),
    };
    assert!(err.is_file_not_found());
}

#[test]
fn is_file_not_found_false_for_other_variants() {
    assert!(!CredentialsBootstrapError::NotInitialized.is_file_not_found());
    assert!(!CredentialsBootstrapError::AlreadyInitialized.is_file_not_found());
    assert!(!CredentialsBootstrapError::NotAvailable.is_file_not_found());
    assert!(!CredentialsBootstrapError::TokenExpired.is_file_not_found());
    assert!(
        !CredentialsBootstrapError::InvalidCredentials {
            message: "x".to_string()
        }
        .is_file_not_found()
    );
    assert!(
        !CredentialsBootstrapError::ApiValidationFailed {
            message: "x".to_string()
        }
        .is_file_not_found()
    );
}

#[test]
fn debug_format_not_initialized() {
    let err = CredentialsBootstrapError::NotInitialized;
    let d = format!("{err:?}");
    assert!(d.contains("NotInitialized"));
}

#[test]
fn debug_format_file_not_found() {
    let err = CredentialsBootstrapError::FileNotFound {
        path: "/tmp/creds.json".to_string(),
    };
    let d = format!("{err:?}");
    assert!(d.contains("FileNotFound"));
    assert!(d.contains("/tmp/creds.json"));
}

#[test]
fn into_cloud_error_not_initialized_yields_credentials_not_initialized() {
    let cloud_err: CloudError = CredentialsBootstrapError::NotInitialized.into();
    assert!(matches!(cloud_err, CloudError::CredentialsNotInitialized));
}

#[test]
fn into_cloud_error_already_initialized_yields_credentials_already_initialized() {
    let cloud_err: CloudError = CredentialsBootstrapError::AlreadyInitialized.into();
    assert!(matches!(
        cloud_err,
        CloudError::CredentialsAlreadyInitialized
    ));
}

#[test]
fn into_cloud_error_not_available_yields_not_authenticated() {
    let cloud_err: CloudError = CredentialsBootstrapError::NotAvailable.into();
    assert!(matches!(cloud_err, CloudError::NotAuthenticated));
}

#[test]
fn into_cloud_error_file_not_found_preserves_path() {
    let cloud_err: CloudError = CredentialsBootstrapError::FileNotFound {
        path: "/tmp/z.json".to_string(),
    }
    .into();
    match cloud_err {
        CloudError::CredentialsFileNotFound { path } => {
            assert_eq!(path, "/tmp/z.json");
        },
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn into_cloud_error_invalid_credentials_preserves_message() {
    let cloud_err: CloudError = CredentialsBootstrapError::InvalidCredentials {
        message: "bad field".to_string(),
    }
    .into();
    match cloud_err {
        CloudError::InvalidCredentials { message } => {
            assert_eq!(message, "bad field");
        },
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn into_cloud_error_token_expired_yields_token_expired() {
    let cloud_err: CloudError = CredentialsBootstrapError::TokenExpired.into();
    assert!(matches!(cloud_err, CloudError::TokenExpired));
}

#[test]
fn into_cloud_error_api_validation_failed_preserves_message() {
    let cloud_err: CloudError = CredentialsBootstrapError::ApiValidationFailed {
        message: "rejected".to_string(),
    }
    .into();
    match cloud_err {
        CloudError::ApiValidationFailed { message } => {
            assert_eq!(message, "rejected");
        },
        other => panic!("unexpected variant: {other:?}"),
    }
}

#[test]
fn all_conversions_produce_non_empty_display() {
    let errs: Vec<CredentialsBootstrapError> = vec![
        CredentialsBootstrapError::NotInitialized,
        CredentialsBootstrapError::AlreadyInitialized,
        CredentialsBootstrapError::NotAvailable,
        CredentialsBootstrapError::TokenExpired,
        CredentialsBootstrapError::FileNotFound {
            path: "/p".to_string(),
        },
        CredentialsBootstrapError::InvalidCredentials {
            message: "m".to_string(),
        },
        CredentialsBootstrapError::ApiValidationFailed {
            message: "a".to_string(),
        },
    ];
    for err in errs {
        let s = err.to_string();
        assert!(!s.is_empty(), "empty display for {err:?}");
    }
}
