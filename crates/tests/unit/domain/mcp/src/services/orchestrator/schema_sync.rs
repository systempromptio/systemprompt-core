//! Per-server schema validation during MCP orchestration.
//!
//! This runs before servers start, and how loudly it fails is decided by
//! `settings.schema_validation_mode`. Under the default `auto_migrate` a schema
//! problem is a warning and the start proceeds; only `strict` turns it into an
//! error that aborts. Both directions are covered here, because reading the
//! wrong one into an incident report is how a start that "succeeded" gets
//! mistaken for a start that validated.

use systemprompt_mcp::test_api::{validate_and_migrate_schemas, validate_schemas};
use systemprompt_models::mcp::deployment::SchemaDefinition;
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap};

use crate::harness::{bootstrap_with_services, internal_mcp_config};

const STRICT_SERVICES_YAML: &str = r"settings:
  schema_validation_mode: strict
mcp_servers: {}
";

fn server_with_schema(name: &str) -> systemprompt_models::mcp::McpServerConfig {
    let mut config = internal_mcp_config(name, 0);
    config.schemas = vec![SchemaDefinition {
        file: "001_init.sql".to_owned(),
        table: format!("{name}_table"),
        required_columns: vec!["id".to_owned()],
    }];
    config
}

#[tokio::test]
async fn a_run_with_no_servers_reports_nothing_validated_and_nothing_created() {
    let _ = ensure_test_bootstrap();
    let pool = closed_db_pool().await;

    let report = validate_and_migrate_schemas(&[], &pool)
        .await
        .expect("an empty server list is not a failure");

    assert_eq!(report.validated, 0);
    assert_eq!(report.created, 0);
    assert!(report.errors.is_empty());
    assert_eq!(
        report.service_name, "all",
        "the combined report covers the whole run"
    );
}

// Why: most MCP servers declare no schemas at all. They must be skipped before
// the validator is asked to do anything, or every start pays for a database
// round trip per server that owns no tables.
#[tokio::test]
async fn servers_declaring_no_schemas_are_skipped_rather_than_validated() {
    let _ = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let servers = [
        internal_mcp_config("no_schema_a", 0),
        internal_mcp_config("no_schema_b", 0),
    ];

    let report = validate_and_migrate_schemas(&servers, &pool)
        .await
        .expect("skipping is not a failure");

    assert!(
        report.errors.is_empty() && report.warnings.is_empty(),
        "a server with no schemas must not reach the database: {report:?}"
    );
    assert_eq!(report.validated, 0);
}

// Why: this is the auto-migrate contract, and it is the surprising half. An
// unreachable database is recorded as a WARNING, not an error, and the start is
// allowed to proceed — the mode exists so a missing table gets created rather
// than blocking a boot. The warning still has to name the offending table, or
// the only trace of the problem is unattributable.
#[tokio::test]
async fn under_auto_migrate_a_schema_failure_is_a_warning_that_names_its_table() {
    let _ = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let servers = [server_with_schema("schema_warn_one")];

    let report = validate_and_migrate_schemas(&servers, &pool)
        .await
        .expect("auto_migrate collects rather than aborts");

    assert!(
        report.errors.is_empty(),
        "auto_migrate must not populate errors: {:?}",
        report.errors
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("schema_warn_one_table")),
        "the unreachable table must be named in the warnings: {:?}",
        report.warnings
    );
}

// Why: the consequence of the above. `validate_schemas` is what the
// orchestrator calls, and under auto_migrate it must return Ok even though the
// database was unreachable — otherwise the mode would block exactly the boots
// it exists to unblock.
#[tokio::test]
async fn under_auto_migrate_the_run_is_not_aborted_by_a_schema_failure() {
    let _ = ensure_test_bootstrap();
    let pool = closed_db_pool().await;

    validate_schemas(&[server_with_schema("schema_warn_two")], &pool)
        .await
        .expect("auto_migrate must not abort the start");
}

#[tokio::test]
async fn a_run_with_nothing_to_validate_succeeds() {
    let _ = ensure_test_bootstrap();
    let pool = closed_db_pool().await;

    validate_schemas(&[internal_mcp_config("clean", 0)], &pool)
        .await
        .expect("a server owning no tables must not block the start");
}

// Why: strict is the opposite contract and the one an operator opts into when a
// wrong schema must stop the boot. Here the failure is an error, it names the
// server that caused it, and it aborts with a count so a multi-server failure
// is not reported as one.
//
// This test owns its process's bootstrap: `schema_validation_mode` is read from
// the services tree through a memoised loader, so the mode cannot be changed
// after the default bootstrap has run. Under nextest each test is its own
// process, which is what makes both modes testable in one file.
#[tokio::test]
async fn under_strict_a_schema_failure_aborts_and_names_the_server() {
    let _ = bootstrap_with_services(STRICT_SERVICES_YAML);
    let pool = closed_db_pool().await;
    let servers = [
        server_with_schema("schema_strict_a"),
        server_with_schema("schema_strict_b"),
    ];

    let report = validate_and_migrate_schemas(&servers, &pool)
        .await
        .expect("collecting errors is not itself a failure");

    assert_eq!(
        report.errors.len(),
        2,
        "strict records one error per failing server: {:?}",
        report.errors
    );
    assert!(
        report.errors.iter().any(|e| e.contains("schema_strict_a"))
            && report.errors.iter().any(|e| e.contains("schema_strict_b")),
        "each failing server must be named: {:?}",
        report.errors
    );

    let err = validate_schemas(&servers, &pool)
        .await
        .expect_err("strict must abort the start");
    let message = err.to_string();
    assert!(
        message.contains("Schema validation failed"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains('2'),
        "both failures must be counted, got: {message}"
    );
}
