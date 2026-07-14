//! Tests for CliService methods not covered by the basic smoke suite.
//!
//! Covers: status_line, session_context, session_context_with_url,
//! profile_banner, and table delegation.

use systemprompt_identifiers::SessionId;
use systemprompt_logging::CliService;
use systemprompt_logging::services::cli::theme::ItemStatus;

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
    CliService::session_context_with_url("dev", &sid, Some("acme"), Some("http://localhost:8080"));
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
fn table_delegates_to_render_table() {
    CliService::table(
        &["Name", "Status"],
        &[
            vec!["alpha".to_owned(), "ok".to_owned()],
            vec!["beta".to_owned(), "fail".to_owned()],
        ],
    );
}
