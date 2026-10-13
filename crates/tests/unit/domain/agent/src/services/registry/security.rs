use std::collections::HashMap;
use systemprompt_agent::models::a2a::{
    ApiKeyLocation, ApiKeySecurityScheme, AuthorizationCodeOAuthFlow, ClientCredentialsOAuthFlow,
    OAuth2SecurityScheme, OAuthFlows, SecurityScheme,
};
use systemprompt_agent::services::registry::security::{
    oauth_to_security_config, override_oauth_urls, requirements_from_config,
};
use systemprompt_manifest::AgentOAuthConfig;
use systemprompt_models::auth::{JwtAudience, Permission};

fn auth_code(scheme: &SecurityScheme) -> &AuthorizationCodeOAuthFlow {
    match scheme {
        SecurityScheme::OAuth2(OAuth2SecurityScheme {
            flows: OAuthFlows::AuthorizationCode(flow),
            ..
        }) => flow,
        other => panic!("expected an OAuth2 authorization-code scheme, got {other:?}"),
    }
}

fn auth_code_scheme(authorize: &str, token: &str, refresh: &str) -> SecurityScheme {
    SecurityScheme::OAuth2(OAuth2SecurityScheme {
        description: None,
        flows: OAuthFlows::AuthorizationCode(AuthorizationCodeOAuthFlow {
            authorization_url: authorize.to_owned(),
            token_url: token.to_owned(),
            refresh_url: Some(refresh.to_owned()),
            scopes: HashMap::new(),
            pkce_required: false,
        }),
        oauth2_metadata_url: None,
    })
}

#[test]
fn oauth_required_yields_oauth2_scheme_and_requirement() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![Permission::Admin, Permission::User],
        audience: JwtAudience::A2a,
    };

    let (schemes, reqs) = oauth_to_security_config(&oauth, "https://api.example.com");

    assert!(schemes.expect("schemes").contains_key("oauth2"));
    let reqs = reqs.expect("requirements");
    assert_eq!(reqs.len(), 1);
    let scopes = &reqs[0].schemes["oauth2"].list;
    assert!(scopes.contains(&"admin".to_owned()));
    assert!(scopes.contains(&"user".to_owned()));
}

#[test]
fn oauth_not_required_yields_nothing() {
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
fn oauth_urls_and_description_are_built_from_the_external_url() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![],
        audience: JwtAudience::Resource("my-audience".to_owned()),
    };

    let (schemes, _) = oauth_to_security_config(&oauth, "https://myhost.com");
    let schemes = schemes.expect("schemes");
    let scheme = schemes.get("oauth2").expect("oauth2");
    let flow = auth_code(scheme);
    assert_eq!(
        flow.authorization_url,
        "https://myhost.com/api/v1/core/oauth/authorize"
    );
    assert_eq!(flow.token_url, "https://myhost.com/api/v1/core/oauth/token");
    assert_eq!(
        flow.refresh_url.as_deref(),
        Some("https://myhost.com/api/v1/core/oauth/token")
    );
    let SecurityScheme::OAuth2(oauth2) = scheme else {
        panic!("expected OAuth2");
    };
    assert!(
        oauth2
            .description
            .as_deref()
            .expect("description")
            .contains("my-audience")
    );
}

#[test]
fn oauth_scopes_are_described() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![Permission::User, Permission::Admin],
        audience: JwtAudience::A2a,
    };

    let (schemes, _) = oauth_to_security_config(&oauth, "https://example.com");
    let schemes = schemes.expect("schemes");
    let flow = auth_code(schemes.get("oauth2").expect("oauth2"));
    assert_eq!(flow.scopes.len(), 2);
    assert!(flow.scopes["user"].contains("access"));
    assert!(flow.scopes.contains_key("admin"));
}

#[test]
fn oauth_empty_scopes_yield_empty_requirement_list() {
    let oauth = AgentOAuthConfig {
        required: true,
        scopes: vec![],
        audience: JwtAudience::A2a,
    };

    let (schemes, reqs) = oauth_to_security_config(&oauth, "https://api.example.com");
    let schemes = schemes.expect("schemes");
    assert!(
        auth_code(schemes.get("oauth2").expect("oauth2"))
            .scopes
            .is_empty()
    );
    assert!(
        reqs.expect("requirements")[0].schemes["oauth2"]
            .list
            .is_empty()
    );
}

#[test]
fn override_prepends_external_url_to_relative_paths() {
    let mut schemes = HashMap::from([(
        "oauth2".to_owned(),
        auth_code_scheme("/oauth/authorize", "/oauth/token", "/oauth/refresh"),
    )]);

    override_oauth_urls(&mut schemes, "https://api.example.com");

    let flow = auth_code(&schemes["oauth2"]);
    assert_eq!(
        flow.authorization_url,
        "https://api.example.com/oauth/authorize"
    );
    assert_eq!(flow.token_url, "https://api.example.com/oauth/token");
    assert_eq!(
        flow.refresh_url.as_deref(),
        Some("https://api.example.com/oauth/refresh")
    );
}

#[test]
fn override_leaves_absolute_urls_unchanged() {
    let mut schemes = HashMap::from([(
        "oauth2".to_owned(),
        auth_code_scheme(
            "https://external.auth.com/authorize",
            "https://external.auth.com/token",
            "https://external.auth.com/refresh",
        ),
    )]);

    override_oauth_urls(&mut schemes, "https://api.example.com");

    let flow = auth_code(&schemes["oauth2"]);
    assert_eq!(
        flow.authorization_url,
        "https://external.auth.com/authorize"
    );
    assert_eq!(flow.token_url, "https://external.auth.com/token");
}

#[test]
fn override_resolves_client_credentials_token_url() {
    let mut schemes = HashMap::from([(
        "oauth2".to_owned(),
        SecurityScheme::OAuth2(OAuth2SecurityScheme {
            description: None,
            flows: OAuthFlows::ClientCredentials(ClientCredentialsOAuthFlow {
                token_url: "/oauth/token".to_owned(),
                refresh_url: None,
                scopes: HashMap::new(),
            }),
            oauth2_metadata_url: None,
        }),
    )]);

    override_oauth_urls(&mut schemes, "https://api.example.com");

    let SecurityScheme::OAuth2(OAuth2SecurityScheme {
        flows: OAuthFlows::ClientCredentials(flow),
        ..
    }) = &schemes["oauth2"]
    else {
        panic!("expected client-credentials flow");
    };
    assert_eq!(flow.token_url, "https://api.example.com/oauth/token");
}

#[test]
fn override_without_oauth2_key_is_noop() {
    let api_key = SecurityScheme::ApiKey(ApiKeySecurityScheme {
        description: None,
        location: ApiKeyLocation::Header,
        name: "X-API-Key".to_owned(),
    });
    let mut schemes = HashMap::from([("apiKey".to_owned(), api_key.clone())]);

    override_oauth_urls(&mut schemes, "https://api.example.com");

    assert_eq!(schemes.len(), 1);
    assert_eq!(schemes["apiKey"], api_key);
}

#[test]
fn requirements_from_config_maps_each_entry_to_one_requirement() {
    let config = vec![
        HashMap::from([("oauth2".to_owned(), vec!["admin".to_owned()])]),
        HashMap::from([("apiKey".to_owned(), vec![]), ("mtls".to_owned(), vec![])]),
    ];

    let reqs = requirements_from_config(&config);

    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].schemes["oauth2"].list, vec!["admin".to_owned()]);
    assert_eq!(reqs[1].schemes.len(), 2);
    assert!(reqs[1].schemes["apiKey"].list.is_empty());
}
