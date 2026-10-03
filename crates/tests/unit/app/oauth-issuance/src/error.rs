use systemprompt_oauth_issuance::IssuanceError;

#[test]
fn test_token_error_invalid_request_display() {
    let error = IssuanceError::InvalidRequest {
        field: "redirect_uri".to_string(),
        message: "is required".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("redirect_uri"));
    assert!(display.contains("is required"));
}

#[test]
fn test_token_error_unsupported_grant_type_display() {
    let error = IssuanceError::UnsupportedGrantType {
        grant_type: "implicit".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("implicit"));
}

#[test]
fn test_token_error_invalid_client_display() {
    let error = IssuanceError::InvalidClient;

    let display = format!("{error}");
    assert!(display.contains("Invalid client credentials"));
}

#[test]
fn test_token_error_invalid_grant_display() {
    let error = IssuanceError::InvalidGrant {
        reason: "code already used".to_string(),
    };

    let display = format!("{error}");
    assert!(display.contains("code already used"));
}

#[test]
fn test_token_error_expired_code_display() {
    let error = IssuanceError::ExpiredCode;

    let display = format!("{error}");
    assert!(display.contains("expired"));
}

#[test]
fn test_token_error_server_error_display() {
    let error = IssuanceError::server(
        "Token generation failed",
        std::io::Error::other("database unavailable"),
    );

    let display = format!("{error}");
    assert!(display.contains("Token generation failed"));
    assert!(std::error::Error::source(&error).is_some());
}
