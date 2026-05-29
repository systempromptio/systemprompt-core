use systemprompt_agent::models::web::{
    AgentCardInput, CreateAgentRequest, CreateAgentRequestRaw, UpdateAgentRequest,
    UpdateAgentRequestRaw,
};
use systemprompt_agent::models::a2a::{AgentCapabilities, TransportProtocol};
use serde_json::json;

fn deserialize_create_request(json_val: serde_json::Value) -> CreateAgentRequest {
    serde_json::from_value(json_val).expect("should deserialize")
}

fn deserialize_update_request(json_val: serde_json::Value) -> UpdateAgentRequest {
    serde_json::from_value(json_val).expect("should deserialize")
}

#[test]
fn create_request_deserialize_with_url() {
    let json = json!({
        "card": {
            "name": "my-agent",
            "description": "Test",
            "version": "1.0.0",
            "url": "http://localhost:8080"
        }
    });
    let req = deserialize_create_request(json);
    assert_eq!(req.card.name, "my-agent");
    assert_eq!(req.card.version, "1.0.0");
}

#[test]
fn create_request_deserialize_without_url_uses_placeholder() {
    let json = json!({
        "card": {
            "name": "my-agent",
            "description": "Test",
            "version": "1.0.0"
        }
    });
    let req = deserialize_create_request(json);
    let url = req.card.url().unwrap_or("");
    assert!(url.contains("my-agent"));
}

#[test]
fn create_request_is_active_defaults_true() {
    let json = json!({
        "card": {
            "name": "agent",
            "description": "Desc",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    assert!(req.is_active());
}

#[test]
fn create_request_is_active_can_be_false() {
    let json = json!({
        "card": {
            "name": "agent",
            "description": "Desc",
            "version": "1.0.0",
            "url": "http://localhost"
        },
        "is_active": false
    });
    let req = deserialize_create_request(json);
    assert!(!req.is_active());
}

#[test]
fn create_request_get_version() {
    let json = json!({
        "card": {
            "name": "versioned",
            "description": "D",
            "version": "2.3.4",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    assert_eq!(req.get_version(), "2.3.4");
}

#[test]
fn create_request_extract_port_from_url() {
    let json = json!({
        "card": {
            "name": "port-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost:9000"
        }
    });
    let req = deserialize_create_request(json);
    assert_eq!(req.extract_port(), 9000);
}

#[test]
fn create_request_extract_port_default_http() {
    let json = json!({
        "card": {
            "name": "port-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://example.com"
        }
    });
    let req = deserialize_create_request(json);
    assert_eq!(req.extract_port(), 80);
}

#[test]
fn create_request_get_capabilities() {
    let json = json!({
        "card": {
            "name": "cap-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    let caps = req.get_capabilities();
    let _ = caps;
}

#[test]
fn create_request_from_raw_with_api_url() {
    let raw = CreateAgentRequestRaw {
        card: minimal_card_input(),
        is_active: None,
        system_prompt: Some("You are helpful".to_string()),
        mcp_servers: None,
    };
    let req = CreateAgentRequest::from_raw(raw, "https://api.example.com");
    let url = req.card.url().unwrap_or("");
    assert!(url.starts_with("https://api.example.com"));
}

#[test]
fn create_request_from_raw_with_explicit_url() {
    let mut card = minimal_card_input();
    card.url = Some("http://custom:3000".to_string());
    let raw = CreateAgentRequestRaw {
        card,
        is_active: Some(true),
        system_prompt: None,
        mcp_servers: Some(vec![]),
    };
    let req = CreateAgentRequest::from_raw(raw, "https://ignored.example.com");
    assert_eq!(req.card.url(), Some("http://custom:3000"));
}

#[test]
fn create_request_default_input_modes_fallback() {
    let json = json!({
        "card": {
            "name": "modes-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    assert!(req.card.default_input_modes.contains(&"text/plain".to_string()));
}

#[test]
fn create_request_with_explicit_input_modes() {
    let json = json!({
        "card": {
            "name": "modes-agent2",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost",
            "default_input_modes": ["text/markdown", "application/json"]
        }
    });
    let req = deserialize_create_request(json);
    assert!(req.card.default_input_modes.contains(&"text/markdown".to_string()));
    assert!(!req.card.default_input_modes.contains(&"text/plain".to_string()));
}

#[test]
fn create_request_preferred_transport_defaults_to_json_rpc() {
    let json = json!({
        "card": {
            "name": "transport-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    let iface = &req.card.supported_interfaces[0];
    assert!(matches!(iface.protocol_binding, TransportProtocol::JsonRpc));
}

#[tokio::test]
async fn create_request_validate_empty_name_fails() {
    let json = json!({
        "card": {
            "name": "",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    let result = req.validate().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Name is required"));
}

#[tokio::test]
async fn create_request_validate_bad_url_fails() {
    let json = json!({
        "card": {
            "name": "valid-name",
            "description": "D",
            "version": "1.0.0",
            "url": "ftp://bad-protocol"
        }
    });
    let req = deserialize_create_request(json);
    let result = req.validate().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn create_request_validate_bad_version_fails() {
    let json = json!({
        "card": {
            "name": "valid-name",
            "description": "D",
            "version": "not-semver",
            "url": "http://localhost"
        }
    });
    let req = deserialize_create_request(json);
    let result = req.validate().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Version must be"));
}

#[test]
fn update_request_is_active_defaults_true() {
    let json = json!({
        "card": {
            "name": "upd-agent",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_update_request(json);
    assert!(req.is_active());
}

#[test]
fn update_request_extract_port() {
    let json = json!({
        "card": {
            "name": "upd-port",
            "description": "D",
            "version": "1.0.0",
            "url": "https://example.com:443"
        }
    });
    let req = deserialize_update_request(json);
    assert_eq!(req.extract_port(), 443);
}

#[test]
fn update_request_from_raw_with_api_url() {
    let raw = UpdateAgentRequestRaw {
        card: minimal_card_input(),
        is_active: Some(false),
        system_prompt: None,
        mcp_servers: None,
    };
    let req = UpdateAgentRequest::from_raw(raw, "http://api.local");
    assert!(!req.is_active());
    let url = req.card.url().unwrap_or("");
    assert!(url.starts_with("http://api.local"));
}

#[tokio::test]
async fn update_request_validate_empty_name_fails() {
    let json = json!({
        "card": {
            "name": "",
            "description": "D",
            "version": "1.0.0",
            "url": "http://localhost"
        }
    });
    let req = deserialize_update_request(json);
    let result = req.validate().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn update_request_validate_bad_url_fails() {
    let json = json!({
        "card": {
            "name": "valid",
            "description": "D",
            "version": "1.0.0",
            "url": "not-http"
        }
    });
    let req = deserialize_update_request(json);
    let result = req.validate().await;
    assert!(result.is_err());
}

fn minimal_card_input() -> AgentCardInput {
    AgentCardInput {
        protocol_version: "0.2.9".to_string(),
        name: "test-agent".to_string(),
        description: "A test".to_string(),
        url: None,
        version: "1.0.0".to_string(),
        preferred_transport: None,
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec![],
        default_output_modes: vec![],
        skills: vec![],
        security_schemes: None,
        security: None,
    }
}
