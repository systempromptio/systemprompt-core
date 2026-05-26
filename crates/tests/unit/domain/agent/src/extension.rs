//! Unit tests for the AgentExtension trait implementation.
//!
//! Targets:
//! - crates/domain/agent/src/extension.rs

use systemprompt_agent::AgentExtension;
use systemprompt_extension::prelude::Extension;

#[test]
fn metadata_basics() {
    let ext = AgentExtension;
    let meta = ext.metadata();
    assert_eq!(meta.id, "agent");
    assert_eq!(meta.name, "Agent");
    assert!(!meta.version.is_empty());
}

#[test]
fn schemas_contain_all_tables() {
    let schemas = AgentExtension.schemas();
    let table_names: Vec<String> = schemas.iter().map(|s| s.table.clone()).collect();

    assert!(table_names.iter().any(|n| n == "user_contexts"));
    assert!(table_names.iter().any(|n| n == "agent_tasks"));
    assert!(table_names.iter().any(|n| n == "task_messages"));
    assert!(table_names.iter().any(|n| n == "message_parts"));
    assert!(table_names.iter().any(|n| n == "task_artifacts"));
    assert!(table_names.iter().any(|n| n == "artifact_parts"));
    assert!(table_names.iter().any(|n| n == "context_agents"));
    assert!(table_names.iter().any(|n| n == "context_notifications"));
    assert!(table_names.iter().any(|n| n == "task_push_notification_configs"));
    assert!(table_names.iter().any(|n| n == "task_execution_steps"));
    assert!(table_names.iter().any(|n| n == "services"));
    assert!(table_names.iter().any(|n| n == "user_session_analytics"));
}

#[test]
fn dependencies_declared() {
    let deps = AgentExtension.dependencies();
    assert!(deps.contains(&"users"));
    assert!(deps.contains(&"oauth"));
    assert!(deps.contains(&"mcp"));
    assert!(deps.contains(&"ai"));
}

#[test]
fn cross_extension_tables_includes_ai_requests() {
    let xtables = AgentExtension.cross_extension_tables();
    assert!(xtables.contains(&"ai_requests"));
}

#[test]
fn migrations_smoke() {
    let _migrations = AgentExtension.migrations();
}

#[test]
fn default_construction() {
    let ext = AgentExtension::default();
    assert_eq!(ext.metadata().id, "agent");
}
