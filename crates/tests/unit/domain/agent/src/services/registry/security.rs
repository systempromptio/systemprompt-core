use std::collections::HashMap;
use systemprompt_agent::SecurityScheme;
use systemprompt_agent::services::registry::security::{
    oauth_to_security_config, override_oauth_urls,
};
use systemprompt_models::AgentOAuthConfig;
use systemprompt_models::auth::{JwtAudience, Permission};

#[test]
fn test_oauth_to_security_config_required() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![Permission::Admin, Permission::User],
        audience: JwtAudience::A2a,
    };

    let (schemes, reqs) = oauth_to_security_config(&oauth, "https://api.example.com");

    let schemes = schemes.expect("expected Some");
    assert!(schemes.contains_key("oauth2"));

    let reqs = reqs.expect("expected Some");
    assert_eq!(reqs.len(), 1);
    assert!(reqs[0].contains_key("oauth2"));
    let scopes = &reqs[0]["oauth2"];
    assert!(scopes.contains(&"admin".to_string()));
    assert!(scopes.contains(&"user".to_string()));
}

#[test]
fn test_oauth_to_security_config_not_required() {
    let oauth = AgentOAuthConfig {
        required: false,
        scopes: vec![Permission::Admin],
        audience: JwtAudience::A2a,
    };

    let (schemes, reqs) = oauth_to_security_config(&oauth, "https://api.example.com");

    assert!(schemes.is_none());
    assert!(reqs.is_none());
}

#[test]
fn test_oauth_to_security_config_urls_constructed_correctly() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![],
        audience: JwtAudience::Resource("my-audience".to_string()),
    };

    let (schemes, _) = oauth_to_security_config(&oauth, "https://myhost.com");

    let schemes = schemes.expect("expected Some");
    match schemes.get("oauth2").expect("expected oauth2 key") {
        SecurityScheme::OAuth2 { flows, description } => {
            let auth_code = flows
                .authorization_code
                .as_ref()
                .expect("expected auth code flow");
            assert_eq!(
                auth_code.authorization_url.as_deref(),
                Some("https://myhost.com/api/v1/core/oauth/authorize")
            );
            assert_eq!(
                auth_code.token_url.as_deref(),
                Some("https://myhost.com/api/v1/core/oauth/token")
            );
            assert_eq!(
                auth_code.refresh_url.as_deref(),
                Some("https://myhost.com/api/v1/core/oauth/token")
            );
            let desc = description.as_ref().expect("expected description");
            assert!(desc.contains("my-audience"));
        },
        _ => panic!("Expected OAuth2 variant"),
    }
}

#[test]
fn test_oauth_to_security_config_scopes_mapped() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![Permission::User, Permission::Admin],
        audience: JwtAudience::A2a,
    };

    let (schemes, _) = oauth_to_security_config(&oauth, "https://example.com");

    let schemes = schemes.expect("expected Some");
    match schemes.get("oauth2").expect("expected oauth2") {
        SecurityScheme::OAuth2 { flows, .. } => {
            let auth_code = flows
                .authorization_code
                .as_ref()
                .expect("expected auth code");
            assert_eq!(auth_code.scopes.len(), 2);
            assert!(auth_code.scopes.contains_key("user"));
            assert!(auth_code.scopes.contains_key("admin"));
            assert!(auth_code.scopes["user"].contains("access"));
        },
        _ => panic!("Expected OAuth2 variant"),
    }
}

#[test]
fn test_override_oauth_urls_relative_paths_get_prepended() {
    let mut schemes = HashMap::new();
    schemes.insert(
        "oauth2".to_string(),
        SecurityScheme::OAuth2 {
            flows: Box::new(systemprompt_agent::models::a2a::OAuth2Flows {
                authorization_code: Some(systemprompt_agent::models::a2a::OAuth2Flow {
                    authorization_url: Some("/oauth/authorize".to_string()),
                    token_url: Some("/oauth/token".to_string()),
                    refresh_url: Some("/oauth/refresh".to_string()),
                    scopes: HashMap::new(),
                }),
                implicit: None,
                password: None,
                client_credentials: None,
            }),
            description: None,
        },
    );

    override_oauth_urls(&mut schemes, "https://api.example.com");

    match schemes.get("oauth2").expect("expected oauth2") {
        SecurityScheme::OAuth2 { flows, .. } => {
            let auth_code = flows
                .authorization_code
                .as_ref()
                .expect("expected auth code");
            assert_eq!(
                auth_code.authorization_url.as_deref(),
                Some("https://api.example.com/oauth/authorize")
            );
            assert_eq!(
                auth_code.token_url.as_deref(),
                Some("https://api.example.com/oauth/token")
            );
            assert_eq!(
                auth_code.refresh_url.as_deref(),
                Some("https://api.example.com/oauth/refresh")
            );
        },
        _ => panic!("Expected OAuth2 variant"),
    }
}

#[test]
fn test_override_oauth_urls_absolute_urls_unchanged() {
    let mut schemes = HashMap::new();
    schemes.insert(
        "oauth2".to_string(),
        SecurityScheme::OAuth2 {
            flows: Box::new(systemprompt_agent::models::a2a::OAuth2Flows {
                authorization_code: Some(systemprompt_agent::models::a2a::OAuth2Flow {
                    authorization_url: Some("https://external.auth.com/authorize".to_string()),
                    token_url: Some("https://external.auth.com/token".to_string()),
                    refresh_url: Some("https://external.auth.com/refresh".to_string()),
                    scopes: HashMap::new(),
                }),
                implicit: None,
                password: None,
                client_credentials: None,
            }),
            description: None,
        },
    );

    override_oauth_urls(&mut schemes, "https://api.example.com");

    match schemes.get("oauth2").expect("expected oauth2") {
        SecurityScheme::OAuth2 { flows, .. } => {
            let auth_code = flows
                .authorization_code
                .as_ref()
                .expect("expected auth code");
            assert_eq!(
                auth_code.authorization_url.as_deref(),
                Some("https://external.auth.com/authorize")
            );
            assert_eq!(
                auth_code.token_url.as_deref(),
                Some("https://external.auth.com/token")
            );
        },
        _ => panic!("Expected OAuth2 variant"),
    }
}

#[test]
fn test_override_oauth_urls_no_oauth2_key_is_noop() {
    let mut schemes = HashMap::new();
    schemes.insert(
        "apiKey".to_string(),
        SecurityScheme::ApiKey {
            name: "X-API-Key".to_string(),
            location: systemprompt_agent::models::a2a::ApiKeyLocation::Header,
            description: None,
        },
    );

    override_oauth_urls(&mut schemes, "https://api.example.com");

    assert!(schemes.contains_key("apiKey"));
    assert!(!schemes.contains_key("oauth2"));
}

#[test]
fn test_override_oauth_urls_no_authorization_code_flow() {
    let mut schemes = HashMap::new();
    schemes.insert(
        "oauth2".to_string(),
        SecurityScheme::OAuth2 {
            flows: Box::new(systemprompt_agent::models::a2a::OAuth2Flows {
                authorization_code: None,
                implicit: None,
                password: None,
                client_credentials: None,
            }),
            description: None,
        },
    );

    override_oauth_urls(&mut schemes, "https://api.example.com");

    match schemes.get("oauth2").expect("expected oauth2") {
        SecurityScheme::OAuth2 { flows, .. } => {
            assert!(flows.authorization_code.is_none());
        },
        _ => panic!("Expected OAuth2 variant"),
    }
}

#[test]
fn test_override_oauth_urls_empty_schemes_map() {
    let mut schemes: HashMap<String, SecurityScheme> = HashMap::new();
    override_oauth_urls(&mut schemes, "https://api.example.com");
    assert!(schemes.is_empty());
}

#[test]
fn test_oauth_to_security_config_empty_scopes() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![],
        audience: JwtAudience::A2a,
    };

    let (schemes, reqs) = oauth_to_security_config(&oauth, "https://api.example.com");

    let schemes = schemes.expect("expected Some");
    match schemes.get("oauth2").expect("expected oauth2") {
        SecurityScheme::OAuth2 { flows, .. } => {
            let auth_code = flows
                .authorization_code
                .as_ref()
                .expect("expected auth code");
            assert!(auth_code.scopes.is_empty());
        },
        _ => panic!("Expected OAuth2 variant"),
    }

    let reqs = reqs.expect("expected Some");
    assert!(reqs[0]["oauth2"].is_empty());
}
