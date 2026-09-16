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
    let table_names: Vec<String> = schemas.iter().filter_map(|s| s.table.clone()).collect();
    assert_eq!(table_names.len(), 9);

    assert!(table_names.iter().any(|n| n == "user_contexts"));
    assert!(table_names.iter().any(|n| n == "agent_tasks"));
    assert!(table_names.iter().any(|n| n == "task_messages"));
    assert!(table_names.iter().any(|n| n == "message_parts"));
    assert!(table_names.iter().any(|n| n == "task_artifacts"));
    assert!(table_names.iter().any(|n| n == "artifact_parts"));
    assert!(table_names.iter().any(|n| n == "context_agents"));
    assert!(table_names.iter().any(|n| n == "context_notifications"));
    assert!(table_names.iter().any(|n| n == "task_execution_steps"));
    assert!(!table_names.iter().any(|n| n == "services"));
    assert!(
        AgentExtension
            .cross_extension_tables()
            .contains(&"services")
    );
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

#[test]
fn owner_capture_and_privacy_contracts_are_registered() {
    let schemas = AgentExtension.schemas();
    let capture: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("EXECUTE FUNCTION sp_capture_reporting_change")
        })
        .collect();
    assert_eq!(
        capture.len(),
        1,
        "owner capture SQL must be registered exactly once"
    );
    for view in [
        "reporting_source_agent_tasks",
        "reporting_source_task_messages",
        "reporting_source_user_contexts",
    ] {
        assert!(
            capture[0].sql.contains(view),
            "missing reporting view: {view}"
        );
    }
    let privacy: Vec<_> = schemas
        .iter()
        .filter(|schema| {
            schema.table.is_none()
                && schema
                    .sql
                    .contains("CREATE OR REPLACE FUNCTION public.lock_agent_reporting_sources")
        })
        .collect();
    assert_eq!(
        privacy.len(),
        1,
        "owner privacy SQL must survive capture registration"
    );
    assert!(privacy[0].sql.contains("reporting_task_is_retained"));
}
