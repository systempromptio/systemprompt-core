//! Unit tests for CreateAgentRequest and UpdateAgentRequest.
//!
//! Tests cover the deserialize / from_raw / validate / accessor paths in:
//! - crates/domain/agent/src/models/web/create_agent.rs
//! - crates/domain/agent/src/models/web/update_agent.rs

use systemprompt_agent::models::a2a::TransportProtocol;
use systemprompt_agent::models::web::{
    CreateAgentRequest, CreateAgentRequestRaw, UpdateAgentRequest, UpdateAgentRequestRaw,
};

fn create_json_minimal() -> serde_json::Value {
    serde_json::json!({
        "card": {
            "name": "test-agent",
            "description": "An agent for testing",
            "version": "1.0.0",
        }
    })
}

fn create_json_full() -> serde_json::Value {
    serde_json::json!({
        "card": {
            "name": "full-agent",
            "description": "Full agent",
            "version": "2.3.4",
            "url": "https://example.com:8443/api/v1/agents/full-agent",
            "preferred_transport": "GRPC",
            "protocol_version": "1.2.3",
            "default_input_modes": ["text/plain", "application/json"],
            "default_output_modes": ["text/plain"],
            "skills": [],
        },
        "is_active": false,
        "system_prompt": "be helpful",
        "mcp_servers": [],
    })
}

#[test]
fn test_create_agent_request_deserialize_minimal() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();

    assert_eq!(request.card.name, "test-agent");
    assert_eq!(request.card.description, "An agent for testing");
    assert_eq!(request.card.version, "1.0.0");
    assert_eq!(
        request.card.url().unwrap(),
        "http://placeholder/api/v1/agents/test-agent"
    );
    assert_eq!(request.card.default_input_modes, vec!["text/plain"]);
    assert_eq!(request.card.default_output_modes, vec!["text/plain"]);
    assert_eq!(
        request.card.supported_interfaces[0].protocol_binding,
        TransportProtocol::JsonRpc
    );
    assert!(request.is_active.is_none());
    assert!(request.system_prompt.is_none());
    assert!(request.mcp_servers.is_none());
}

#[test]
fn test_create_agent_request_deserialize_full() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_full()).unwrap();

    assert_eq!(request.card.name, "full-agent");
    assert_eq!(request.card.version, "2.3.4");
    assert_eq!(
        request.card.url().unwrap(),
        "https://example.com:8443/api/v1/agents/full-agent"
    );
    assert_eq!(
        request.card.supported_interfaces[0].protocol_binding,
        TransportProtocol::Grpc
    );
    assert_eq!(request.card.default_input_modes.len(), 2);
    assert_eq!(request.is_active, Some(false));
    assert_eq!(request.system_prompt.as_deref(), Some("be helpful"));
    assert_eq!(request.mcp_servers, Some(vec![]));
}

#[test]
fn test_create_agent_request_from_raw_with_url() {
    let raw: CreateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "my-agent",
            "description": "desc",
            "version": "1.0.0",
            "url": "https://override/api/v1/agents/my-agent",
        }
    }))
    .unwrap();
    let request = CreateAgentRequest::from_raw(raw, "http://api-server");

    assert_eq!(
        request.card.url().unwrap(),
        "https://override/api/v1/agents/my-agent"
    );
}

#[test]
fn test_create_agent_request_from_raw_without_url_uses_server() {
    let raw: CreateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "my-agent",
            "description": "desc",
            "version": "1.0.0",
        }
    }))
    .unwrap();
    let request = CreateAgentRequest::from_raw(raw, "http://my-server:9999");

    assert_eq!(
        request.card.url().unwrap(),
        "http://my-server:9999/api/v1/agents/my-agent"
    );
}

#[test]
fn test_create_agent_request_from_raw_input_modes_default() {
    let raw: CreateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "agent",
            "description": "d",
            "version": "1.0.0",
        }
    }))
    .unwrap();
    let request = CreateAgentRequest::from_raw(raw, "http://x");

    assert_eq!(request.card.default_input_modes, vec!["text/plain"]);
    assert_eq!(request.card.default_output_modes, vec!["text/plain"]);
}

#[test]
fn test_create_agent_request_from_raw_input_modes_preserved() {
    let raw: CreateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "agent",
            "description": "d",
            "version": "1.0.0",
            "default_input_modes": ["audio/mpeg"],
            "default_output_modes": ["image/png", "image/jpeg"],
        }
    }))
    .unwrap();
    let request = CreateAgentRequest::from_raw(raw, "http://x");

    assert_eq!(request.card.default_input_modes, vec!["audio/mpeg"]);
    assert_eq!(
        request.card.default_output_modes,
        vec!["image/png", "image/jpeg"]
    );
}

#[tokio::test]
async fn test_create_agent_validate_empty_name() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.name = "   ".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("Name is required"));
}

#[tokio::test]
async fn test_create_agent_validate_bad_url() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.supported_interfaces[0].url = "ftp://example.com/foo".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("URL must be a valid HTTP or HTTPS URL"));
}

#[tokio::test]
async fn test_create_agent_validate_bad_version() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.version = "not-a-version".to_string();
    request.card.supported_interfaces[0].url = "http://example.com".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("semantic version"));
}

#[tokio::test]
async fn test_create_agent_validate_ok_without_mcp() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.supported_interfaces[0].url = "http://example.com".to_string();
    request.mcp_servers = None;

    let result = request.validate().await;
    assert!(result.is_ok(), "expected validate() ok, got {:?}", result);
}

#[tokio::test]
async fn test_create_agent_validate_ok_empty_mcp_servers() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.supported_interfaces[0].url = "https://example.com:443".to_string();
    request.mcp_servers = Some(Vec::new());

    let result = request.validate().await;
    assert!(result.is_ok(), "expected validate() ok, got {:?}", result);
}

#[test]
fn test_create_agent_get_version() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    assert_eq!(request.get_version(), "1.0.0");
}

#[test]
fn test_create_agent_is_active_default() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    assert!(request.is_active());
}

#[test]
fn test_create_agent_is_active_false() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.is_active = Some(false);
    assert!(!request.is_active());
}

#[test]
fn test_create_agent_extract_port_default() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    assert_eq!(request.extract_port(), 80);
}

#[test]
fn test_create_agent_extract_port_explicit() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.supported_interfaces[0].url = "http://example.com:9001/path".to_string();
    assert_eq!(request.extract_port(), 9001);
}

#[test]
fn test_create_agent_extract_port_https_default() {
    let mut request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    request.card.supported_interfaces[0].url = "https://example.com/x".to_string();
    assert_eq!(request.extract_port(), 443);
}

#[test]
fn test_create_agent_get_capabilities() {
    let request: CreateAgentRequest = serde_json::from_value(create_json_minimal()).unwrap();
    let _caps = request.get_capabilities();
    assert_eq!(request.card.name, "test-agent");
}

#[test]
fn test_update_agent_request_deserialize_minimal() {
    let json = serde_json::json!({
        "card": {
            "name": "upd-agent",
            "description": "desc",
            "version": "0.1.0",
        }
    });
    let request: UpdateAgentRequest = serde_json::from_value(json).unwrap();

    assert_eq!(request.card.name, "upd-agent");
    assert_eq!(
        request.card.url().unwrap(),
        "http://placeholder/api/v1/agents/upd-agent"
    );
    assert_eq!(
        request.card.supported_interfaces[0].protocol_binding,
        TransportProtocol::JsonRpc
    );
    assert!(request.is_active.is_none());
}

#[test]
fn test_update_agent_from_raw_with_url() {
    let raw: UpdateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
            "url": "https://upd.example/api/v1/agents/upd",
            "preferred_transport": "HTTP+JSON",
        }
    }))
    .unwrap();
    let request = UpdateAgentRequest::from_raw(raw, "http://server");
    assert_eq!(
        request.card.url().unwrap(),
        "https://upd.example/api/v1/agents/upd"
    );
    assert_eq!(
        request.card.supported_interfaces[0].protocol_binding,
        TransportProtocol::HttpJson
    );
}

#[test]
fn test_update_agent_from_raw_without_url() {
    let raw: UpdateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "upd-2",
            "description": "d",
            "version": "1.0.0",
        }
    }))
    .unwrap();
    let request = UpdateAgentRequest::from_raw(raw, "http://api-srv:7000");
    assert_eq!(
        request.card.url().unwrap(),
        "http://api-srv:7000/api/v1/agents/upd-2"
    );
}

#[test]
fn test_update_agent_from_raw_modes_default_when_empty() {
    let raw: UpdateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    }))
    .unwrap();
    let request = UpdateAgentRequest::from_raw(raw, "http://x");
    assert_eq!(request.card.default_input_modes, vec!["text/plain"]);
    assert_eq!(request.card.default_output_modes, vec!["text/plain"]);
}

#[test]
fn test_update_agent_from_raw_modes_preserved() {
    let raw: UpdateAgentRequestRaw = serde_json::from_value(serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
            "default_input_modes": ["audio/wav"],
            "default_output_modes": ["application/octet-stream"],
        }
    }))
    .unwrap();
    let request = UpdateAgentRequest::from_raw(raw, "http://x");
    assert_eq!(request.card.default_input_modes, vec!["audio/wav"]);
    assert_eq!(
        request.card.default_output_modes,
        vec!["application/octet-stream"]
    );
}

#[tokio::test]
async fn test_update_agent_validate_empty_name() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    });
    let mut request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    request.card.name = "".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("Name is required"));
}

#[tokio::test]
async fn test_update_agent_validate_empty_endpoint() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    });
    let mut request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    request.card.supported_interfaces[0].url = "  ".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("Endpoint"));
}

#[tokio::test]
async fn test_update_agent_validate_bad_endpoint_scheme() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    });
    let mut request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    request.card.supported_interfaces[0].url = "tcp://example.com".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("HTTP or HTTPS URL"));
}

#[tokio::test]
async fn test_update_agent_validate_bad_version() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "bad-version",
        }
    });
    let mut request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    request.card.supported_interfaces[0].url = "http://example.com".to_string();

    let err = request.validate().await.unwrap_err();
    assert!(err.contains("semantic version"));
}

#[tokio::test]
async fn test_update_agent_validate_ok() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.2.3",
        }
    });
    let mut request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    request.card.supported_interfaces[0].url = "https://example.com:8443/path".to_string();
    request.mcp_servers = None;

    let result = request.validate().await;
    assert!(result.is_ok(), "expected validate() ok, got {:?}", result);
}

#[test]
fn test_update_agent_is_active_defaults_to_true() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    });
    let request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    assert!(request.is_active());
}

#[test]
fn test_update_agent_is_active_false() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        },
        "is_active": false,
    });
    let request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    assert!(!request.is_active());
}

#[test]
fn test_update_agent_extract_port_default_http() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
        }
    });
    let request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    assert_eq!(request.extract_port(), 80);
}

#[test]
fn test_update_agent_extract_port_explicit() {
    let json = serde_json::json!({
        "card": {
            "name": "upd",
            "description": "d",
            "version": "1.0.0",
            "url": "http://localhost:12345/api/v1/agents/upd",
        }
    });
    let request: UpdateAgentRequest = serde_json::from_value(json).unwrap();
    assert_eq!(request.extract_port(), 12345);
}
