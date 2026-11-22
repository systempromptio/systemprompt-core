//! What `McpExtension` contributes: its seven tables, its jobs and retention
//! purge, the three values it provides, and the two slots it opens so their
//! built values exist whenever the member is admitted.

use std::any::TypeId;
use std::sync::Arc;

use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_identifiers::ExtensionId;
use systemprompt_mcp::McpExtension;
use systemprompt_mcp::contract::{AccessRecorderSlot, UiSlot};
use systemprompt_mcp::services::McpServiceObserver;
use systemprompt_traits::{DynToolExecutionLookup, McpRegistryProvider};

fn contributed() -> ContributionSink {
    let mut sink = ContributionSink::default();
    let mut c = Contributions::__new(ExtensionId::new("mcp"), &mut sink);
    McpExtension
        .contribute(&mut c)
        .expect("the mcp member contributes");
    sink
}

#[test]
fn metadata_names_the_mcp_member() {
    let meta = McpExtension.metadata();
    assert_eq!(meta.id, "mcp");
    assert_eq!(meta.name, "MCP");
    assert!(!meta.version.is_empty());
}

#[test]
fn the_mcp_member_declares_its_seven_tables() {
    let sink = contributed();
    let names: Vec<&str> = sink
        .schemas()
        .iter()
        .filter_map(|s| s.table.as_deref())
        .collect();
    assert_eq!(names.len(), 7, "{names:?}");
    for table in [
        "mcp_tool_executions",
        "mcp_sessions",
        "mcp_artifacts",
        "mcp_external_sessions",
        "tool_result_ingest_marks",
    ] {
        assert!(names.contains(&table), "{table} in {names:?}");
    }
    assert!(!names.contains(&"mcp_proxy_identities"));
}

#[test]
fn the_mcp_member_contributes_its_jobs_and_purge() {
    let sink = contributed();
    assert_eq!(sink.jobs().len(), 2, "{:?}", sink.jobs());
    assert_eq!(sink.retention().len(), 1);
}

#[test]
fn the_mcp_member_provides_the_execution_lookup_the_registry_the_observer_and_the_tool_executions()
{
    let sink = contributed();
    let provided: Vec<TypeId> = sink.provides().iter().map(|p| p.type_id()).collect();
    assert_eq!(
        provided,
        [
            TypeId::of::<DynToolExecutionLookup>(),
            TypeId::of::<Arc<dyn McpRegistryProvider>>(),
            TypeId::of::<Arc<McpServiceObserver>>(),
            TypeId::of::<systemprompt_extension::mcp::DynToolExecutions>(),
        ]
    );
}

#[test]
fn the_mcp_member_opens_the_process_ui_and_access_recorder_slots() {
    let sink = contributed();
    let slots: Vec<TypeId> = sink.slots().iter().map(|record| record.slot()).collect();
    assert_eq!(
        slots,
        [
            TypeId::of::<systemprompt_extension::service_process::ServiceProcessSlot>(),
            TypeId::of::<UiSlot>(),
            TypeId::of::<AccessRecorderSlot>(),
        ]
    );
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
