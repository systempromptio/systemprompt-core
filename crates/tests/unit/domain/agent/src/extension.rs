//! What `AgentExtension` contributes: its nine tables, the cross-member
//! tables it references, its migrations and its two jobs.

use systemprompt_agent::AgentExtension;
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_identifiers::ExtensionId;

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("agent"), &mut sink);
    AgentExtension
        .contribute(&mut c)
        .expect("the agent member contributes");
    sink
}

#[test]
fn metadata_names_the_agent_member() {
    let meta = AgentExtension.metadata();
    assert_eq!(meta.id, "agent");
    assert_eq!(meta.name, "Agent");
    assert!(!meta.version.is_empty());
}

#[test]
fn the_agent_member_declares_its_tables() {
    let sink = contributed();
    let tables: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect();
    assert_eq!(tables.len(), 9, "{tables:?}");
    for table in [
        "user_contexts",
        "agent_tasks",
        "task_messages",
        "message_parts",
        "task_artifacts",
        "artifact_parts",
        "context_agents",
        "context_notifications",
        "task_execution_steps",
    ] {
        assert!(tables.contains(&table), "{table} in {tables:?}");
    }
    assert!(!tables.contains(&"services"));
}

#[test]
fn the_agent_member_references_ai_requests_and_services() {
    assert_eq!(contributed().cross_tables(), ["ai_requests", "services"]);
}

#[test]
fn the_agent_member_contributes_its_jobs() {
    let sink = contributed();
    let mut names: Vec<&str> = sink.jobs().iter().map(|job| job.job().name()).collect();
    names.sort_unstable();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"user_context_counters_rollup"), "{names:?}");
}

#[test]
fn the_agent_member_contributes_migrations() {
    assert!(!contributed().migrations().is_empty());
}

#[test]
fn no_reporting_capture_or_privacy_sql_is_contributed() {
    assert!(
        contributed()
            .schemas()
            .iter()
            .all(|schema| !schema.sql.contains("reporting")),
        "the reporting projection is retired"
    );
}
