use serde_json::json;
use systemprompt_models::a2a::task_metadata::{TaskMetadata, TaskType, agent_names};
use systemprompt_models::execution::ExecutionStep;
use systemprompt_traits::validation::Validate;

#[test]
fn agent_names_system_constant() {
    assert_eq!(agent_names::SYSTEM, "system");
}

#[test]
fn task_type_serde_snake_case() {
    let json = serde_json::to_string(&TaskType::McpExecution).unwrap();
    assert_eq!(json, "\"mcp_execution\"");
    let back: TaskType = serde_json::from_str(&json).unwrap();
    assert_eq!(back, TaskType::McpExecution);

    let json2 = serde_json::to_string(&TaskType::AgentMessage).unwrap();
    assert_eq!(json2, "\"agent_message\"");
}

#[test]
fn new_mcp_execution_populates_required_fields() {
    let m = TaskMetadata::new_mcp_execution("agent".into(), "tool".into(), "srv".into());
    assert_eq!(m.task_type, TaskType::McpExecution);
    assert_eq!(m.agent_name, "agent");
    assert_eq!(m.tool_name.as_deref(), Some("tool"));
    assert_eq!(m.mcp_server_name.as_deref(), Some("srv"));
    assert!(!m.created_at.is_empty());
    assert!(m.updated_at.is_none());
    assert!(m.execution_steps.is_none());
    assert!(m.extensions.is_none());
}

#[test]
fn new_agent_message_populates_required_fields() {
    let m = TaskMetadata::new_agent_message("agent".into());
    assert_eq!(m.task_type, TaskType::AgentMessage);
    assert!(m.tool_name.is_none());
    assert!(m.mcp_server_name.is_none());
    assert!(!m.created_at.is_empty());
}

#[test]
fn builder_methods_compose() {
    let m = TaskMetadata::new_agent_message("agent".into())
        .with_token_usage(11, 22)
        .with_model("gpt-x")
        .with_updated_at()
        .with_tool_name("toolio")
        .with_execution_steps(Vec::<ExecutionStep>::new())
        .with_extension("k".to_owned(), json!(1));
    assert_eq!(m.input_tokens, Some(11));
    assert_eq!(m.output_tokens, Some(22));
    assert_eq!(m.model.as_deref(), Some("gpt-x"));
    assert!(m.updated_at.is_some());
    assert_eq!(m.tool_name.as_deref(), Some("toolio"));
    assert!(m.execution_steps.is_some());
    let ext = m.extensions.as_ref().unwrap();
    assert_eq!(ext.get("k"), Some(&json!(1)));
}

#[test]
fn with_extension_inserts_into_existing_map() {
    let m = TaskMetadata::new_agent_message("a".into())
        .with_extension("a".into(), json!(1))
        .with_extension("b".into(), json!("hi"));
    let ext = m.extensions.as_ref().unwrap();
    assert_eq!(ext.len(), 2);
    assert_eq!(ext.get("a"), Some(&json!(1)));
    assert_eq!(ext.get("b"), Some(&json!("hi")));
}

#[test]
fn validate_succeeds_for_filled_metadata() {
    let m = TaskMetadata::new_agent_message("a".into());
    assert!(m.validate().is_ok());
}

#[test]
fn validate_fails_for_empty_agent_name() {
    let m = TaskMetadata::new_agent_message(String::new());
    assert!(m.validate().is_err());
}

#[test]
fn new_validated_agent_message_rejects_empty_name() {
    let err = TaskMetadata::new_validated_agent_message(String::new()).unwrap_err();
    assert_eq!(err.field, "agent_name");
}

#[test]
fn new_validated_agent_message_accepts_valid_name() {
    let m = TaskMetadata::new_validated_agent_message("ok".into()).unwrap();
    assert_eq!(m.agent_name, "ok");
}

#[test]
fn new_validated_mcp_execution_rejects_empty_agent_name() {
    let err =
        TaskMetadata::new_validated_mcp_execution(String::new(), "tool".into(), "srv".into())
            .unwrap_err();
    assert_eq!(err.field, "agent_name");
}

#[test]
fn new_validated_mcp_execution_rejects_empty_tool_name() {
    let err =
        TaskMetadata::new_validated_mcp_execution("a".into(), String::new(), "srv".into())
            .unwrap_err();
    assert_eq!(err.field, "tool_name");
}

#[test]
fn new_validated_mcp_execution_accepts_valid_inputs() {
    let m = TaskMetadata::new_validated_mcp_execution("a".into(), "t".into(), "s".into()).unwrap();
    assert_eq!(m.task_type, TaskType::McpExecution);
}

#[test]
fn task_metadata_serde_round_trip_omits_none() {
    let m = TaskMetadata::new_agent_message("a".into());
    let json = serde_json::to_value(&m).unwrap();
    assert!(json.get("updated_at").is_none());
    assert!(json.get("started_at").is_none());
    assert!(json.get("execution_time_ms").is_none());
    let back: TaskMetadata = serde_json::from_value(json).unwrap();
    assert_eq!(back, m);
}
