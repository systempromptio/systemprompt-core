use std::collections::HashMap;
use systemprompt_agent::services::registry::security::{
    convert_json_security_to_struct, oauth_to_security_config, override_oauth_urls,
};
use systemprompt_agent::SecurityScheme;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::AgentOAuthConfig;

#[test]
fn test_convert_json_security_none_inputs() {
    let (schemes, reqs) = convert_json_security_to_struct(None, None);

    assert!(schemes.is_none());
    assert!(reqs.is_none());
}

#[test]
fn test_convert_json_security_valid_oauth2_scheme() {
    let schemes_json = serde_json::json!({
        "oauth2": {
            "type": "oauth2",
            "flows": {
                "authorizationCode": {
                    "authorizationUrl": "https://auth.example.com/authorize",
                    "tokenUrl": "https://auth.example.com/token",
                    "scopes": {
                        "read": "Read access"
                    }
                }
            }
        }
    });

    let (schemes, _) = convert_json_security_to_struct(Some(&schemes_json), None);

    let schemes = schemes.expect("expected Some");
    assert!(schemes.contains_key("oauth2"));
}

#[test]
fn test_convert_json_security_valid_requirements() {
    let reqs_json = vec![serde_json::json!({"oauth2": ["read", "write"]})];

    let (_, reqs) = convert_json_security_to_struct(None, Some(&reqs_json));

    let reqs = reqs.expect("expected Some");
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0]["oauth2"], vec!["read", "write"]);
}

#[test]
fn test_convert_json_security_invalid_schemes_returns_none() {
    let invalid_json = serde_json::json!("not a map");

    let (schemes, _) = convert_json_security_to_struct(Some(&invalid_json), None);

    assert!(schemes.is_none());
}

#[test]
fn test_convert_json_security_invalid_requirements_returns_none() {
    let invalid_reqs = vec![serde_json::json!("not a map")];

    let (_, reqs) = convert_json_security_to_struct(None, Some(&invalid_reqs));

    assert!(reqs.is_none());
}

#[test]
fn test_convert_json_security_both_valid() {
    let schemes_json = serde_json::json!({
        "apiKey": {
            "type": "apiKey",
            "name": "X-API-Key",
            "in": "header"
        }
    });
    let reqs_json = vec![serde_json::json!({"apiKey": []})];

    let (schemes, reqs) = convert_json_security_to_struct(Some(&schemes_json), Some(&reqs_json));

    assert!(schemes.is_some());
    assert!(reqs.is_some());
}

#[test]
fn test_convert_json_security_empty_schemes_map() {
    let empty_json = serde_json::json!({});

    let (schemes, _) = convert_json_security_to_struct(Some(&empty_json), None);

    let schemes = schemes.expect("expected Some");
    assert!(schemes.is_empty());
}

#[test]
fn test_convert_json_security_empty_requirements_list() {
    let empty_reqs: Vec<serde_json::Value> = vec![];

    let (_, reqs) = convert_json_security_to_struct(None, Some(&empty_reqs));

    let reqs = reqs.expect("expected Some");
    assert!(reqs.is_empty());
}

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
            let auth_code = flows.authorization_code.as_ref().expect("expected auth code flow");
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
            let auth_code = flows.authorization_code.as_ref().expect("expected auth code");
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
            let auth_code = flows.authorization_code.as_ref().expect("expected auth code");
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
            let auth_code = flows.authorization_code.as_ref().expect("expected auth code");
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
            let auth_code = flows.authorization_code.as_ref().expect("expected auth code");
            assert!(auth_code.scopes.is_empty());
        },
        _ => panic!("Expected OAuth2 variant"),
    }

    let reqs = reqs.expect("expected Some");
    assert!(reqs[0]["oauth2"].is_empty());
}

#[test]
fn test_convert_json_security_multiple_requirements() {
    let reqs_json = vec![
        serde_json::json!({"oauth2": ["read"]}),
        serde_json::json!({"apiKey": []}),
    ];

    let (_, reqs) = convert_json_security_to_struct(None, Some(&reqs_json));

    let reqs = reqs.expect("expected Some");
    assert_eq!(reqs.len(), 2);
    assert!(reqs[0].contains_key("oauth2"));
    assert!(reqs[1].contains_key("apiKey"));
}

#[test]
fn test_convert_json_security_http_bearer_scheme() {
    let schemes_json = serde_json::json!({
        "bearer": {
            "type": "http",
            "scheme": "bearer",
            "bearerFormat": "JWT"
        }
    });

    let (schemes, _) = convert_json_security_to_struct(Some(&schemes_json), None);

    let schemes = schemes.expect("expected Some");
    assert!(schemes.contains_key("bearer"));
    match schemes.get("bearer").unwrap() {
        SecurityScheme::Http { scheme, bearer_format, .. } => {
            assert_eq!(scheme, "bearer");
            let _ = bearer_format;
        },
        _ => panic!("Expected Http variant"),
    }
}
