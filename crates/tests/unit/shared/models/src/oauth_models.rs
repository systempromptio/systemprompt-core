use systemprompt_identifiers::ClientId;
use systemprompt_models::oauth::{OAuthClientConfig, OAuthServerConfig};

#[test]
fn oauth_client_config_new_sets_required_fields() {
    let c = OAuthClientConfig::new(
        "github",
        ClientId::new("client_123"),
        "https://github.com/login/oauth/authorize",
        "https://github.com/login/oauth/access_token",
    );
    assert_eq!(c.provider, "github");
    assert_eq!(c.client_id, ClientId::new("client_123"));
    assert_eq!(c.authorization_url, "https://github.com/login/oauth/authorize");
    assert_eq!(c.token_url, "https://github.com/login/oauth/access_token");
    assert!(c.client_secret.is_none());
    assert!(c.redirect_uri.is_none());
    assert!(c.scopes.is_empty());
}

#[test]
fn oauth_client_config_builder_chains() {
    let c = OAuthClientConfig::new(
        "google",
        ClientId::new("cid"),
        "https://accounts.google.com/o/oauth2/auth",
        "https://oauth2.googleapis.com/token",
    )
    .with_secret("super_secret")
    .with_redirect_uri("https://myapp.com/callback")
    .with_scopes(vec!["openid".to_owned(), "email".to_owned()]);

    assert_eq!(c.client_secret.as_deref(), Some("super_secret"));
    assert_eq!(c.redirect_uri.as_deref(), Some("https://myapp.com/callback"));
    assert_eq!(c.scopes, vec!["openid", "email"]);
}

#[test]
fn oauth_client_config_serde_round_trip() {
    let c = OAuthClientConfig::new(
        "provider",
        ClientId::new("c1"),
        "https://auth.example.com/authorize",
        "https://auth.example.com/token",
    )
    .with_secret("s3cr3t");
    let json = serde_json::to_string(&c).unwrap();
    let decoded: OAuthClientConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.provider, "provider");
    assert_eq!(decoded.client_secret.as_deref(), Some("s3cr3t"));
}

#[test]
fn oauth_server_config_new_sets_issuer() {
    let s = OAuthServerConfig::new("https://auth.example.com");
    assert_eq!(s.issuer, "https://auth.example.com");
    assert!(s.authorization_endpoint.is_empty());
    assert_eq!(s.token_endpoint_auth_method, "client_secret_basic");
    assert_eq!(s.default_scope, "openid");
    assert_eq!(s.auth_code_expiry_seconds, 600);
    assert_eq!(s.access_token_expiry_seconds, 3600);
}

#[test]
fn oauth_server_config_from_api_server_url() {
    let s = OAuthServerConfig::from_api_server_url("https://api.example.com");
    assert!(s.authorization_endpoint.contains("/oauth/authorize"));
    assert!(s.token_endpoint.contains("/oauth/token"));
    assert!(s.registration_endpoint.contains("/oauth/register"));
    assert!(s.supported_scopes.contains(&"user".to_owned()));
    assert!(s.supported_scopes.contains(&"admin".to_owned()));
    assert!(s.supported_grant_types.contains(&"authorization_code".to_owned()));
    assert!(s.supported_code_challenge_methods.contains(&"S256".to_owned()));
}

#[test]
fn oauth_server_config_default_uses_localhost() {
    let s = OAuthServerConfig::default();
    assert_eq!(s.issuer, "http://localhost:8080");
}

#[test]
fn oauth_server_config_serde_round_trip_with_defaults() {
    let s = OAuthServerConfig::from_api_server_url("https://api.example.com");
    let json = serde_json::to_string(&s).unwrap();
    let decoded: OAuthServerConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.issuer, s.issuer);
    assert_eq!(decoded.supported_scopes, s.supported_scopes);
}

#[test]
fn oauth_server_config_serde_default_fields_on_empty_json() {
    let json = r#"{
        "issuer": "https://x.com",
        "authorization_endpoint": "",
        "token_endpoint": "",
        "registration_endpoint": "",
        "supported_scopes": [],
        "supported_grant_types": [],
        "supported_response_types": []
    }"#;
    let s: OAuthServerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(s.token_endpoint_auth_method, "client_secret_basic");
    assert_eq!(s.default_scope, "openid");
    assert_eq!(s.auth_code_expiry_seconds, 600);
    assert!(s.supported_code_challenge_methods.is_empty());
}
