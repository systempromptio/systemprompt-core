//! Smoke tests for `CliService` static helpers.
//!
//! All of these write to stdout/stderr; we only assert they do not panic and
//! return the expected types. Goal is line coverage of the brand-colour and
//! `Theme` wrapping paths.

use systemprompt_logging::CliService;

#[test]
fn message_level_helpers_no_panic() {
    CliService::success("hello");
    CliService::warning("hello");
    CliService::error("hello");
    CliService::info("hello");
    CliService::debug("hello");
    CliService::verbose("hello");
}

#[test]
fn section_and_subsection() {
    CliService::section("Title");
    CliService::subsection("Sub");
}

#[test]
fn clear_screen_and_output() {
    CliService::clear_screen();
    CliService::output("plain content");
}

#[test]
fn json_helpers_pretty_and_compact() {
    let value = serde_json::json!({"k": 1});
    CliService::json(&value);
    CliService::json_compact(&value);
}

#[test]
fn yaml_helper() {
    CliService::yaml(&serde_json::json!({"k": 1}));
}

#[test]
fn key_value_renders() {
    CliService::key_value("kind", "service");
}

#[test]
fn spinner_and_progress_bar_construct() {
    let _spin = CliService::spinner("loading");
    let _bar = CliService::progress_bar(100);
}

#[test]
fn phase_helpers() {
    CliService::startup_banner(Some("dev"));
    CliService::phase("bootstrap");
    CliService::phase_success("started", Some("0.5s"));
    CliService::phase_info("note", None);
    CliService::phase_warning("careful", Some("port in use"));
}

#[test]
fn service_spinner_with_and_without_port() {
    let _ = CliService::service_spinner("api", Some(8080));
    let _ = CliService::service_spinner("worker", None);
}
