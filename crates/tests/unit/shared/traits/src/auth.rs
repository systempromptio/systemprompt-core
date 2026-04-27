use systemprompt_traits::AuthProviderError;

#[test]
fn error_display_messages() {
    assert_eq!(
        AuthProviderError::InvalidCredentials.to_string(),
        "Invalid credentials"
    );
    assert_eq!(
        AuthProviderError::UserNotFound.to_string(),
        "User not found"
    );
    assert_eq!(AuthProviderError::InvalidToken.to_string(), "Invalid token");
    assert_eq!(AuthProviderError::TokenExpired.to_string(), "Token expired");
    assert_eq!(
        AuthProviderError::InsufficientPermissions.to_string(),
        "Insufficient permissions"
    );
}

#[test]
fn internal_error_includes_message() {
    let err = AuthProviderError::Internal("Database connection failed".to_string());
    assert_eq!(
        err.to_string(),
        "Internal error: Database connection failed"
    );
}

#[test]
fn from_anyhow_error() {
    let anyhow_err = anyhow::anyhow!("Something went wrong");
    let auth_err: AuthProviderError = anyhow_err.into();

    match auth_err {
        AuthProviderError::Internal(msg) => {
            assert!(msg.contains("Something went wrong"));
        },
        _ => panic!("Expected Internal error variant"),
    }
}
