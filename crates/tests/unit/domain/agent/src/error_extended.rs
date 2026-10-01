// Additional coverage for error variants not exercised by error.rs:
// ArtifactError::InvalidSchema, the caller-authored AgentError variants and
// the typed config cause.

use std::error::Error as _;

use systemprompt_agent::{AgentError, ArtifactError};

fn json_err() -> serde_json::Error {
    serde_json::from_str::<serde_json::Value>("{ not json").unwrap_err()
}

#[test]
fn artifact_error_invalid_schema_display() {
    let err = ArtifactError::InvalidSchema {
        expected: "ToolResponse",
        actual_keys: vec!["foo".to_string(), "bar".to_string()],
        source: json_err(),
    };
    let msg = err.to_string();
    assert!(msg.contains("Invalid tool response schema"));
    assert!(msg.contains("ToolResponse"));
    assert!(msg.contains("foo"));
}

#[test]
fn agent_error_config_display() {
    assert!(
        AgentError::Config("bad".to_string())
            .to_string()
            .contains("config")
    );
}

#[test]
fn agent_error_invalid_config_keeps_the_cause() {
    let err = AgentError::invalid_config("invalid cors_allowed_origins entry", json_err());
    assert!(matches!(err, AgentError::InvalidConfig { .. }));
    assert!(
        err.to_string()
            .contains("invalid cors_allowed_origins entry")
    );
    assert!(err.source().is_some());
}

#[test]
fn agent_error_not_found_display() {
    assert!(
        AgentError::NotFound("agent-z".to_string())
            .to_string()
            .contains("agent-z")
    );
}

#[test]
fn agent_error_validation_display() {
    assert!(
        AgentError::Validation("invalid".to_string())
            .to_string()
            .contains("validation")
    );
}

#[test]
fn agent_error_no_available_port_display() {
    let msg = AgentError::NoAvailablePort {
        min: 9000,
        max: 9999,
    }
    .to_string();
    assert!(msg.contains("9000-9999"));
}

#[test]
fn agent_error_io_keeps_the_cause() {
    let err: AgentError = std::io::Error::other("bind failed").into();
    assert!(matches!(err, AgentError::Io(_)));
}
