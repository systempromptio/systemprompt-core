//! What `infra services cleanup` reports before it touches anything: the
//! dry-run count and the closing message. Both are what the operator reads to
//! decide whether to run the destructive form.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::infrastructure::services::cleanup::{
    dry_run_result, format_cleanup_message, no_services_result,
};
use systemprompt_cli::shared::CommandOutput;
use systemprompt_database::ServiceConfig;
use systemprompt_identifiers::InstanceId;

fn service(name: &str, pid: Option<i32>, port: i32) -> ServiceConfig {
    ServiceConfig {
        instance_id: InstanceId::new(format!("instance_{}", uuid::Uuid::new_v4().simple())),
        name: name.to_owned(),
        module_name: "mcp".to_owned(),
        status: "running".to_owned(),
        pid,
        port,
        binary_mtime: None,
        heartbeat_at: "2026-01-01T00:00:00Z".to_owned(),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        updated_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn rendered(out: &CommandOutput) -> String {
    serde_json::to_value(out.artifact()).unwrap().to_string()
}

#[test]
fn a_dry_run_counts_the_api_server_alongside_the_service_rows() {
    let services = vec![
        service("alpha", Some(4242), 5010),
        service("beta", None, 5011),
    ];

    let with_api = dry_run_result(&services, Some(999), 8080, true);
    let without_api = dry_run_result(&services, None, 8080, true);

    assert!(
        rendered(&with_api).contains("Would clean 3 service(s)"),
        "a running API server is a third thing cleanup would stop, got: {}",
        rendered(&with_api)
    );
    assert!(
        rendered(&without_api).contains("Would clean 2 service(s)"),
        "with no API server the count is the service rows alone, got: {}",
        rendered(&without_api)
    );
}

#[test]
fn a_dry_run_is_labelled_as_one_and_reports_nothing_removed() {
    let out = dry_run_result(&[service("alpha", Some(1), 5010)], None, 8080, true);
    let json = rendered(&out);

    assert!(
        json.contains("Service Cleanup (Dry Run)"),
        "the title must mark the run as a dry run, got: {json}"
    );
    assert!(
        json.contains("stale_entries_removed") && json.contains('0'),
        "a dry run must report nothing removed, got: {json}"
    );
}

#[test]
fn an_empty_fleet_reports_no_services_rather_than_a_cleanup_of_zero() {
    let json = rendered(&no_services_result(true, false));

    assert!(
        json.contains("No running services found"),
        "the empty case must say so explicitly, got: {json}"
    );
}

#[test]
fn the_closing_message_counts_what_was_cleaned() {
    assert_eq!(format_cleanup_message(3, true), "Cleaned up 3 services");
    assert_eq!(
        format_cleanup_message(0, true),
        "No running services found",
        "a cleanup that stopped nothing must not claim to have cleaned zero services"
    );
}
