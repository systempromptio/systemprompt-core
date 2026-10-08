use systemprompt_oauth_issuance::{TokenRequest, TokenResponse};

#[test]
fn test_token_request_deserialize_authorization_code() {
    let json = serde_json::json!({
        "grant_type": "authorization_code",
        "code": "abc123",
        "redirect_uri": "https://example.com/callback",
        "client_id": "client-1",
        "client_secret": "secret-1",
        "scope": "openid profile",
        "code_verifier": "verifier-value",
        "resource": "https://api.example.com"
    });

    let request: TokenRequest = serde_json::from_value(json).unwrap();

    assert_eq!(request.grant_type, "authorization_code");
    assert_eq!(request.code.as_deref(), Some("abc123"));
    assert_eq!(
        request.redirect_uri.as_deref(),
        Some("https://example.com/callback")
    );
    assert_eq!(request.client_id.as_deref(), Some("client-1"));
    assert_eq!(request.client_secret.as_deref(), Some("secret-1"));
    assert_eq!(request.scope.as_deref(), Some("openid profile"));
    assert_eq!(request.code_verifier.as_deref(), Some("verifier-value"));
    assert_eq!(request.resource.as_deref(), Some("https://api.example.com"));
    assert!(request.refresh_token.is_none());
}

#[test]
fn test_token_request_deserialize_minimal() {
    let json = serde_json::json!({
        "grant_type": "client_credentials"
    });

    let request: TokenRequest = serde_json::from_value(json).unwrap();

    assert_eq!(request.grant_type, "client_credentials");
    assert!(request.code.is_none());
    assert!(request.redirect_uri.is_none());
    assert!(request.client_id.is_none());
    assert!(request.client_secret.is_none());
    assert!(request.refresh_token.is_none());
    assert!(request.scope.is_none());
    assert!(request.code_verifier.is_none());
    assert!(request.resource.is_none());
}


#[test]
fn test_token_response_serialize_full() {
    let response = TokenResponse {
        access_token: "at_xyz".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: Some("rt_abc".to_string()),
        scope: Some("openid profile".to_string()),
        issued_token_type: None,
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["access_token"], "at_xyz");
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(json["expires_in"], 3600);
    assert_eq!(json["refresh_token"], "rt_abc");
    assert_eq!(json["scope"], "openid profile");
}

#[test]
fn test_token_response_serialize_skip_none() {
    let response = TokenResponse {
        access_token: "at_xyz".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 7200,
        refresh_token: None,
        scope: None,
        issued_token_type: None,
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["access_token"], "at_xyz");
    assert_eq!(json["expires_in"], 7200);
    assert!(json.get("refresh_token").is_none());
    assert!(json.get("scope").is_none());
}
