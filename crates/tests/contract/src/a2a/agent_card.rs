use systemprompt_identifiers::SkillId;
use systemprompt_models::a2a::*;

#[test]
fn agent_card_serializes_required_fields() {
    let card = AgentCard {
        name: "test-agent".to_string(),
        description: "A test agent".to_string(),
        supported_interfaces: vec![AgentInterface {
            url: "https://example.com/a2a".to_string(),
            protocol_binding: ProtocolBinding::JsonRpc,
            tenant: None,
            protocol_version: "1.0".to_string(),
        }],
        version: "1.0.0".to_string(),
        capabilities: AgentCapabilities::default(),
        skills: vec![],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        ..Default::default()
    };

    let json = serde_json::to_value(&card).unwrap();

    assert!(json["name"].is_string(), "name is required");
    assert!(json["description"].is_string(), "description is required");
    assert!(json["version"].is_string(), "version is required");
    assert!(json["capabilities"].is_object(), "capabilities is required");
    assert!(json["skills"].is_array(), "skills is required");
    assert!(
        json["supportedInterfaces"].is_array(),
        "supportedInterfaces is required"
    );
    super::schema::assert_valid("AgentCard", &json);
}

#[test]
fn capabilities_default_values() {
    let caps = AgentCapabilities::default();
    assert_eq!(caps.streaming, Some(true));
    assert_eq!(caps.push_notifications, Some(false));
    assert_eq!(caps.extended_agent_card, Some(false));
}

#[test]
fn protocol_binding_jsonrpc_serializes_correctly() {
    let binding = ProtocolBinding::JsonRpc;
    let json = serde_json::to_value(&binding).unwrap();
    assert_eq!(json, "JSONRPC");
}

#[test]
fn protocol_binding_grpc_serializes_correctly() {
    let binding = ProtocolBinding::Grpc;
    let json = serde_json::to_value(&binding).unwrap();
    assert_eq!(json, "GRPC");
}

#[test]
fn protocol_binding_http_json_serializes_correctly() {
    let binding = ProtocolBinding::HttpJson;
    let json = serde_json::to_value(&binding).unwrap();
    assert_eq!(json, "HTTP+JSON");
}

#[test]
fn security_scheme_api_key_is_a_oneof_member() {
    let scheme = SecurityScheme::ApiKey(ApiKeySecurityScheme {
        name: "X-API-Key".to_string(),
        location: ApiKeyLocation::Header,
        description: None,
    });
    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["apiKeySecurityScheme"]["name"], "X-API-Key");
    assert_eq!(json["apiKeySecurityScheme"]["location"], "header");
    super::schema::assert_valid("SecurityScheme", &json);
}

#[test]
fn security_scheme_http_is_a_oneof_member() {
    let scheme = SecurityScheme::HttpAuth(HttpAuthSecurityScheme {
        scheme: "bearer".to_string(),
        bearer_format: Some("JWT".to_string()),
        description: None,
    });
    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["httpAuthSecurityScheme"]["scheme"], "bearer");
    super::schema::assert_valid("SecurityScheme", &json);
}

#[test]
fn security_scheme_oauth2_is_a_oneof_member() {
    let scheme = SecurityScheme::OAuth2(OAuth2SecurityScheme {
        flows: OAuthFlows::ClientCredentials(ClientCredentialsOAuthFlow {
            token_url: "https://example.com/token".to_string(),
            refresh_url: None,
            scopes: std::collections::HashMap::new(),
        }),
        description: None,
        oauth2_metadata_url: None,
    });
    let json = serde_json::to_value(&scheme).unwrap();
    assert!(json["oauth2SecurityScheme"]["flows"].is_object());
    super::schema::assert_valid("SecurityScheme", &json);
}

#[test]
fn security_scheme_openid_connect_is_a_oneof_member() {
    let scheme = SecurityScheme::OpenIdConnect(OpenIdConnectSecurityScheme {
        open_id_connect_url: "https://example.com/.well-known/openid-configuration".to_string(),
        description: None,
    });
    let json = serde_json::to_value(&scheme).unwrap();
    assert!(json["openIdConnectSecurityScheme"]["openIdConnectUrl"].is_string());
    super::schema::assert_valid("SecurityScheme", &json);
}

#[test]
fn api_key_location_serializes_to_spec_values() {
    assert_eq!(
        serde_json::to_value(ApiKeyLocation::Query).unwrap(),
        "query"
    );
    assert_eq!(
        serde_json::to_value(ApiKeyLocation::Header).unwrap(),
        "header"
    );
    assert_eq!(
        serde_json::to_value(ApiKeyLocation::Cookie).unwrap(),
        "cookie"
    );
}

#[test]
fn agent_skill_has_required_fields() {
    let skill = AgentSkill {
        id: SkillId::new("search"),
        name: "Web Search".to_string(),
        description: "Search the web".to_string(),
        tags: vec!["search".to_string()],
        examples: None,
        input_modes: None,
        output_modes: None,
        security_requirements: None,
    };
    let json = serde_json::to_value(&skill).unwrap();
    assert!(json["id"].is_string());
    assert!(json["name"].is_string());
    assert!(json["description"].is_string());
    assert!(json["tags"].is_array());
    super::schema::assert_valid("AgentSkill", &json);
}

#[test]
fn agent_card_optional_fields_omitted_when_none() {
    let card = AgentCard::default();
    let json = serde_json::to_value(&card).unwrap();
    assert!(json.get("iconUrl").is_none());
    assert!(json.get("provider").is_none());
    assert!(json.get("documentationUrl").is_none());
    assert!(json.get("securitySchemes").is_none());
    assert!(json.get("securityRequirements").is_none());
    assert!(json.get("signatures").is_none());
}
