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
    let err = AuthProviderError::Internal("Database connection failed".into());
    assert_eq!(
        err.to_string(),
        "Internal error: Database connection failed"
    );
}

#[test]
fn internal_variant_keeps_its_source() {
    let cause = std::io::Error::other("Something went wrong");
    let auth_err = AuthProviderError::Internal(Box::new(cause));

    assert!(std::error::Error::source(&auth_err).is_some());
    match auth_err {
        AuthProviderError::Internal(source) => {
            assert!(source.to_string().contains("Something went wrong"));
        },
        _ => panic!("Expected Internal error variant"),
    }
}
