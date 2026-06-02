// Additional coverage for error variants not exercised by error.rs:
// the JSON-parse From conversions, ArtifactError::InvalidSchema, and the
// string-carrying AgentError variants plus the RepositoryError conversion.

use systemprompt_agent::{AgentError, ArtifactError, ProtocolError, RowParseError, TaskError};

fn json_err() -> serde_json::Error {
    serde_json::from_str::<serde_json::Value>("{ not json").unwrap_err()
}

#[test]
fn row_parse_error_json_parse_display() {
    let err = RowParseError::JsonParse {
        field: "metadata".to_string(),
        source: json_err(),
    };
    let msg = err.to_string();
    assert!(msg.contains("metadata"));
    assert!(msg.contains("JSON parse error"));
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
fn protocol_error_json_parse_from() {
    let err: ProtocolError = json_err().into();
    assert!(matches!(err, ProtocolError::JsonParse(_)));
    assert!(err.to_string().contains("JSON parse error"));
}

#[test]
fn task_error_invalid_metadata_from_json() {
    let err: TaskError = json_err().into();
    assert!(matches!(err, TaskError::InvalidMetadata(_)));
}

#[test]
fn task_error_database_display() {
    let err = TaskError::Database("conn lost".to_string());
    assert!(err.to_string().contains("Database error"));
    assert!(err.to_string().contains("conn lost"));
}

#[test]
fn artifact_error_database_display() {
    let err = ArtifactError::Database("write failed".to_string());
    assert!(err.to_string().contains("Database error"));
}

#[test]
fn agent_error_init_display() {
    let err = AgentError::Init("pool missing".to_string());
    assert!(err.to_string().contains("repository init"));
    assert!(err.to_string().contains("pool missing"));
}

#[test]
fn agent_error_server_display() {
    assert!(
        AgentError::Server("boom".to_string())
            .to_string()
            .contains("server")
    );
}

#[test]
fn agent_error_webhook_display() {
    assert!(
        AgentError::Webhook("hook".to_string())
            .to_string()
            .contains("webhook")
    );
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
fn agent_error_not_found_display() {
    assert!(
        AgentError::NotFound("agent-z".to_string())
            .to_string()
            .contains("agent-z")
    );
}

#[test]
fn agent_error_spawn_display() {
    assert!(
        AgentError::Spawn("no binary".to_string())
            .to_string()
            .contains("spawn failed")
    );
}

#[test]
fn agent_error_lifecycle_display() {
    assert!(
        AgentError::Lifecycle("stuck".to_string())
            .to_string()
            .contains("lifecycle")
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
fn agent_error_internal_display() {
    assert!(
        AgentError::Internal("oops".to_string())
            .to_string()
            .contains("internal")
    );
}

#[test]
fn agent_error_repository_display() {
    assert!(
        AgentError::Repository("repo".to_string())
            .to_string()
            .contains("Repository error")
    );
}

#[test]
fn agent_error_into_repository_error_non_sqlx() {
    let agent_err = AgentError::Internal("boom".to_string());
    let repo_err: systemprompt_traits::RepositoryError = agent_err.into();
    // Non-sqlx variants become RepositoryError::Database carrying the message.
    assert!(format!("{repo_err}").contains("boom"));
}

#[test]
fn agent_error_into_repository_error_sqlx() {
    let agent_err = AgentError::Sqlx(sqlx::Error::RowNotFound);
    let repo_err: systemprompt_traits::RepositoryError = agent_err.into();
    assert!(matches!(
        repo_err,
        systemprompt_traits::RepositoryError::Database(_)
    ));
}

#[test]
fn agent_error_from_task_error_variant() {
    let err: AgentError = TaskError::MissingTaskUuid.into();
    assert!(matches!(err, AgentError::Task(_)));
}
