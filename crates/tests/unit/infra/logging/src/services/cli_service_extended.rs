//! Tests for CliService methods not covered by the basic smoke suite.
//!
//! Covers: status_line, timed, session_context, session_context_with_url,
//! profile_banner, relationship, item, module_status, display helpers,
//! collection builder, and table delegation.

use systemprompt_identifiers::SessionId;
use systemprompt_logging::CliService;
use systemprompt_logging::services::cli::display::{
    CollectionDisplay, Display, ModuleItemDisplay, StatusDisplay,
};
use systemprompt_logging::services::cli::theme::{ItemStatus, ModuleType};

#[test]
fn status_line_all_statuses() {
    CliService::status_line("kind", "api", ItemStatus::Valid);
    CliService::status_line("kind", "db", ItemStatus::Applied);
    CliService::status_line("kind", "cache", ItemStatus::Missing);
    CliService::status_line("kind", "worker", ItemStatus::Failed);
    CliService::status_line("kind", "job", ItemStatus::Disabled);
    CliService::status_line("kind", "queue", ItemStatus::Pending);
}

#[test]
fn timed_runs_closure_and_returns_result() {
    let result = CliService::timed("test-op", || 42_u32);
    assert_eq!(result, 42);
}

#[test]
fn timed_returns_complex_value() {
    let result = CliService::timed("vec-op", || vec![1, 2, 3]);
    assert_eq!(result, vec![1, 2, 3]);
}

#[test]
fn session_context_with_short_session_id() {
    let sid = SessionId::new("short");
    CliService::session_context("local", &sid, None);
    CliService::session_context("prod", &sid, Some("tenant-123"));
}

#[test]
fn session_context_with_long_session_id_truncates() {
    let sid = SessionId::new("very-long-session-id-that-exceeds-twelve-chars");
    CliService::session_context("local", &sid, None);
}

#[test]
fn session_context_with_url() {
    let sid = SessionId::new("sid-abc");
    CliService::session_context_with_url("dev", &sid, None, None);
    CliService::session_context_with_url("dev", &sid, Some("acme"), None);
    CliService::session_context_with_url("dev", &sid, None, Some("http://localhost:8080"));
    CliService::session_context_with_url(
        "dev",
        &sid,
        Some("acme"),
        Some("http://localhost:8080"),
    );
}

#[test]
fn profile_banner_local_no_tenant() {
    CliService::profile_banner("default", false, None);
}

#[test]
fn profile_banner_cloud_with_tenant() {
    CliService::profile_banner("prod", true, Some("acme-corp"));
}

#[test]
fn profile_banner_local_with_tenant() {
    CliService::profile_banner("staging", false, Some("beta"));
}

#[test]
fn relationship_renders_without_panic() {
    CliService::relationship(
        "schema.sql",
        "public.users",
        ItemStatus::Applied,
        ModuleType::Schema,
    );
    CliService::relationship(
        "seed.sql",
        "public.seeds",
        ItemStatus::Missing,
        ModuleType::Seed,
    );
}

#[test]
fn item_with_and_without_detail() {
    CliService::item(ItemStatus::Valid, "module-a", None);
    CliService::item(ItemStatus::Missing, "module-b", Some("not found"));
    CliService::item(ItemStatus::Applied, "module-c", Some("v1.2.3"));
}

#[test]
fn module_status_renders() {
    CliService::module_status("auth", "schemas applied");
    CliService::module_status("database", "up to date");
}

#[test]
fn display_validation_summary_delegates() {
    use systemprompt_logging::services::cli::ValidationSummary;
    let mut s = ValidationSummary::new();
    s.add_valid("a".into(), "1.0".into());
    CliService::display_validation_summary(&s);
}

#[test]
fn display_result_delegates() {
    use systemprompt_logging::services::cli::OperationResult;
    let r = OperationResult::success("migrate");
    CliService::display_result(&r);
    let r2 = OperationResult::failure("migrate", "db not running");
    CliService::display_result(&r2);
}

#[test]
fn display_progress_delegates() {
    use systemprompt_logging::services::cli::ProgressSummary;
    let mut p = ProgressSummary::new("upload", 5);
    p.add_success();
    p.add_success();
    CliService::display_progress(&p);
}

#[test]
fn collection_builder_wraps_display_items() {
    let items: Vec<StatusDisplay> = vec![
        StatusDisplay::new(ItemStatus::Valid, "a"),
        StatusDisplay::new(ItemStatus::Applied, "b"),
    ];
    let col = CliService::collection("Group", items);
    col.display();
}

#[test]
fn table_delegates_to_render_table() {
    CliService::table(
        &["Name", "Status"],
        &[
            vec!["alpha".to_owned(), "ok".to_owned()],
            vec!["beta".to_owned(), "fail".to_owned()],
        ],
    );
}

#[test]
fn status_display_new_and_with_detail() {
    let sd = StatusDisplay::new(ItemStatus::Valid, "module-a");
    assert_eq!(sd.name, "module-a");
    assert!(matches!(sd.status, ItemStatus::Valid));
    assert!(sd.detail.is_none());

    let sd2 = StatusDisplay::new(ItemStatus::Missing, "module-b").with_detail("v2.0");
    assert_eq!(sd2.detail.as_deref(), Some("v2.0"));
}

#[test]
fn status_display_renders_all_statuses() {
    for status in [
        ItemStatus::Valid,
        ItemStatus::Applied,
        ItemStatus::Missing,
        ItemStatus::Failed,
        ItemStatus::Disabled,
        ItemStatus::Pending,
    ] {
        StatusDisplay::new(status, "m").display();
        StatusDisplay::new(status, "m")
            .with_detail("d")
            .display();
    }
}

#[test]
fn module_item_display_all_types() {
    for module_type in [
        ModuleType::Schema,
        ModuleType::Seed,
        ModuleType::Module,
        ModuleType::Configuration,
    ] {
        for status in [ItemStatus::Missing, ItemStatus::Applied, ItemStatus::Valid] {
            ModuleItemDisplay::new(module_type, "file.sql", "target_table", status).display();
        }
    }
}

#[test]
fn collection_display_without_count() {
    let items = vec![
        StatusDisplay::new(ItemStatus::Valid, "x"),
        StatusDisplay::new(ItemStatus::Applied, "y"),
    ];
    let col = CollectionDisplay::new("Items", items).without_count();
    assert!(!col.show_count);
    col.display();
}

#[test]
fn collection_display_empty_does_not_panic() {
    let items: Vec<StatusDisplay> = vec![];
    CollectionDisplay::new("Empty", items).display();
}

#[test]
fn collection_display_empty_without_count() {
    let items: Vec<StatusDisplay> = vec![];
    CollectionDisplay::new("Empty", items).without_count().display();
}
