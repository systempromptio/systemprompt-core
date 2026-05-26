//! Smoke tests for the CLI startup-banner / phase render helpers.
//!
//! These functions write branded text to stderr — we only call them and assert
//! they don't panic. The point is line coverage of the brand-colour wrapping.

use systemprompt_logging::services::cli::startup::{
    render_phase_header, render_phase_info, render_phase_success, render_phase_warning,
    render_startup_banner,
};

#[test]
fn render_startup_banner_with_and_without_subtitle() {
    render_startup_banner(None);
    render_startup_banner(Some("hello"));
}

#[test]
fn render_phase_header_smoke() {
    render_phase_header("bootstrap");
}

#[test]
fn render_phase_status_helpers() {
    render_phase_success("ok", None);
    render_phase_success("ok", Some("done"));
    render_phase_info("info", None);
    render_phase_info("info", Some("note"));
    render_phase_warning("warn", None);
    render_phase_warning("warn", Some("flaky"));
}
