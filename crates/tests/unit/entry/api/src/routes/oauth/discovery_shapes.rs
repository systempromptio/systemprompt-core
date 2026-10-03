use systemprompt_api::routes::oauth::discovery::WellKnownResponse;
use systemprompt_models::oauth::ProtectedResourceMetadata;

#[test]
fn test_well_known_response_serialize() {
    let response = WellKnownResponse {
        issuer: "https://auth.example.com".to_string(),
        authorization_endpoint: "https://auth.example.com/authorize".to_string(),
        token_endpoint: "https://auth.example.com/token".to_string(),
        userinfo_endpoint: "https://auth.example.com/userinfo".to_string(),
        introspection_endpoint: "https://auth.example.com/introspect".to_string(),
        revocation_endpoint: "https://auth.example.com/revoke".to_string(),
        registration_endpoint: Some("https://auth.example.com/register".to_string()),
        scopes_supported: vec!["openid".to_string(), "profile".to_string()],
        response_types_supported: vec!["code".to_string()],
        response_modes_supported: vec!["query".to_string()],
        grant_types_supported: vec!["authorization_code".to_string()],
        token_endpoint_auth_methods_supported: vec!["none".to_string()],
        code_challenge_methods_supported: vec!["S256".to_string()],
        subject_types_supported: vec!["public".to_string()],
        id_token_signing_alg_values_supported: vec!["HS256".to_string()],
        claims_supported: vec!["sub".to_string(), "email".to_string()],
        authorization_response_iss_parameter_supported: true,
        subject_token_types_supported: vec![
            "urn:ietf:params:oauth:token-type:access_token".to_string(),
        ],
        issued_token_types_supported: vec!["urn:ietf:params:oauth:token-type:id-jag".to_string()],
        authorization_grant_profiles_supported: vec![
            "urn:ietf:params:oauth:grant-profile:id-jag".to_owned(),
        ],
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["issuer"], "https://auth.example.com");
    assert_eq!(
        json["authorization_endpoint"],
        "https://auth.example.com/authorize"
    );
    assert_eq!(json["token_endpoint"], "https://auth.example.com/token");
    assert_eq!(
        json["registration_endpoint"],
        "https://auth.example.com/register"
    );
    assert_eq!(json["scopes_supported"].as_array().unwrap().len(), 2);
    assert_eq!(
        json["code_challenge_methods_supported"].as_array().unwrap()[0],
        "S256"
    );
}

#[test]
fn test_oauth_protected_resource_response_serialize() {
    let response = ProtectedResourceMetadata {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec!["https://auth.example.com".to_string()],
        scopes_supported: vec!["read".to_string(), "write".to_string()],
        bearer_methods_supported: vec!["header".to_string()],
        resource_documentation: Some("https://docs.example.com".to_string()),
        mcp_extensions_supported: Vec::new(),
    };

    let json = serde_json::to_value(&response).unwrap();

    assert_eq!(json["resource"], "https://api.example.com");
    assert_eq!(json["authorization_servers"].as_array().unwrap().len(), 1);
    assert_eq!(json["scopes_supported"].as_array().unwrap().len(), 2);
    assert_eq!(json["bearer_methods_supported"][0], "header");
    assert_eq!(json["resource_documentation"], "https://docs.example.com");
}

#[test]
fn test_oauth_protected_resource_response_debug() {
    let response = ProtectedResourceMetadata {
        resource: "https://api.example.com".to_string(),
        authorization_servers: vec![],
        scopes_supported: vec![],
        bearer_methods_supported: vec![],
        resource_documentation: None,
        mcp_extensions_supported: Vec::new(),
    };

    let debug = format!("{response:?}");
    assert!(debug.contains("https://api.example.com"), "{debug}");
}
