//! Schema pin for the execution source.
//!
//! `mcp_tool_executions.source` and `mcp_artifacts.source` are CHECK
//! constraints over the strings `ExecutionSource` writes, and the tool usage
//! report leaves out the rows a host's built-in tools are recorded under by
//! naming the same strings. If any list drifts from the enum, an insert fails
//! or a built-in tool is reported as an MCP server; these tests fail first,
//! against the base schema (fresh installs), the migration (established
//! databases) and the report queries.

use systemprompt_models::mcp::ExecutionSource;

const EXECUTIONS_SCHEMA: &str =
    include_str!("../../../../../domain/mcp/schema/mcp_tool_executions.sql");
const ARTIFACTS_SCHEMA: &str = include_str!("../../../../../domain/mcp/schema/mcp_artifacts.sql");
const MIGRATION_008: &str =
    include_str!("../../../../../domain/mcp/schema/migrations/008_artifact_narrow_waist.sql");
const MIGRATION_022: &str = include_str!(
    "../../../../../domain/mcp/schema/migrations/022_execution_source_hook_harness.sql"
);
const REPORT_LIST: &str =
    include_str!("../../../../../domain/mcp/src/repository/usage_report/list.rs");
const REPORT_DETAIL: &str =
    include_str!("../../../../../domain/mcp/src/repository/usage_report/detail.rs");

const REPORT_EXCLUSION: &str = "server_name NOT IN (";

fn list_after<'a>(sql: &'a str, anchor: &str) -> &'a str {
    let start = sql
        .find(anchor)
        .unwrap_or_else(|| panic!("{anchor} missing"));
    let rest = &sql[start..];
    let open = rest.find("IN (").expect("IN (");
    let close = rest[open..].find(')').expect(")");
    &rest[open..open + close]
}

fn assert_list_is_every_source(list: &str, what: &str) {
    for source in ExecutionSource::ALL {
        assert!(
            list.contains(&format!("'{source}'")),
            "{source} missing from {what}"
        );
    }
    assert_eq!(
        list.matches('\'').count() / 2,
        ExecutionSource::ALL.len(),
        "{what} has extra values"
    );
}

#[test]
fn every_source_is_in_every_check_list() {
    for (sql, anchor, what) in [
        (
            EXECUTIONS_SCHEMA,
            "CHECK (source IN (",
            "mcp_tool_executions base schema",
        ),
        (
            ARTIFACTS_SCHEMA,
            "CHECK (source IN (",
            "mcp_artifacts base schema",
        ),
        (
            MIGRATION_022,
            "ADD CONSTRAINT mcp_tool_executions_source_widened",
            "mcp_tool_executions in migration 022",
        ),
        (
            MIGRATION_022,
            "ADD CONSTRAINT mcp_artifacts_source_widened",
            "mcp_artifacts in migration 022",
        ),
    ] {
        assert_list_is_every_source(list_after(sql, anchor), what);
    }
    for anchor in [
        "ADD CONSTRAINT mcp_tool_executions_source_check",
        "ADD CONSTRAINT mcp_artifacts_source_check",
    ] {
        assert!(
            !list_after(MIGRATION_008, anchor).contains("'hook_harness'"),
            "008 is history; hook_harness arrived in 022"
        );
    }
}

#[test]
fn the_wider_check_takes_the_name_the_base_schema_gives_it() {
    for table in ["mcp_tool_executions", "mcp_artifacts"] {
        assert!(
            MIGRATION_022.contains(&format!(
                "RENAME CONSTRAINT {table}_source_widened TO {table}_source_check;"
            )),
            "{table}: an upgraded database must name the check as a fresh install does"
        );
    }
}

#[test]
fn the_usage_report_leaves_out_every_source_as_a_server_name() {
    let mut lists = 0;
    for (code, file) in [(REPORT_LIST, "list.rs"), (REPORT_DETAIL, "detail.rs")] {
        for (at, _) in code.match_indices(REPORT_EXCLUSION) {
            assert_list_is_every_source(
                list_after(&code[at..], REPORT_EXCLUSION),
                &format!("usage_report/{file} at byte {at}"),
            );
            lists += 1;
        }
    }
    assert_eq!(lists, 7, "six list orders and the overview totals");
}
