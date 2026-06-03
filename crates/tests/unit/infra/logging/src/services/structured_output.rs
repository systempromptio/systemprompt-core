//! Unit tests for structured-output mode and the CLI notice buffer.

use std::sync::Mutex;
use systemprompt_logging::{
    CliService, buffer_notice, drain_notices, mark_structured_emitted, set_structured_output,
    structured_was_emitted,
};

static STRUCTURED_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn buffer_and_drain_round_trip() {
    let _guard = STRUCTURED_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_structured_output(false);
    let _ = drain_notices();

    buffer_notice("warning", "buf-marker-warn");
    buffer_notice("info", "buf-marker-info");
    let drained = drain_notices();

    assert!(
        drained
            .iter()
            .any(|n| n.level == "warning" && n.text == "buf-marker-warn")
    );
    assert!(
        drained
            .iter()
            .any(|n| n.level == "info" && n.text == "buf-marker-info")
    );

    let after = drain_notices();
    assert!(!after.iter().any(|n| n.text == "buf-marker-warn"));
}

#[test]
fn cli_service_notices_buffer_in_structured_mode() {
    let _guard = STRUCTURED_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_structured_output(true);
    let _ = drain_notices();

    CliService::warning("cli-marker-structured");
    let drained = drain_notices();
    set_structured_output(false);

    assert!(
        drained
            .iter()
            .any(|n| n.level == "warning" && n.text == "cli-marker-structured")
    );
}

#[test]
fn cli_service_notices_skip_buffer_when_not_structured() {
    let _guard = STRUCTURED_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    set_structured_output(false);
    let _ = drain_notices();

    CliService::warning("cli-marker-plain");
    let drained = drain_notices();

    assert!(!drained.iter().any(|n| n.text == "cli-marker-plain"));
}

#[test]
fn mark_sets_structured_emitted() {
    let _guard = STRUCTURED_LOCK.lock().unwrap_or_else(|p| p.into_inner());
    mark_structured_emitted();
    assert!(structured_was_emitted());
}
