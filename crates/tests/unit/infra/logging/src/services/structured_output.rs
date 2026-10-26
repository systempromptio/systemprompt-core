//! Unit tests for structured-output mode and the CLI notice buffer.

use systemprompt_logging::{CliService, OutputState};
use tracing_subscriber::layer::SubscriberExt;

fn scoped(output: &OutputState) -> tracing::subscriber::DefaultGuard {
    tracing::subscriber::set_default(tracing_subscriber::registry().with(output.clone()))
}

#[test]
fn buffer_and_drain_round_trip() {
    let output = OutputState::new();
    let _scope = scoped(&output);
    output.set_structured_output(false);
    let _ = output.drain_notices();

    output.buffer_notice("warning", "buf-marker-warn");
    output.buffer_notice("info", "buf-marker-info");
    let drained = output.drain_notices();

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

    let after = output.drain_notices();
    assert!(!after.iter().any(|n| n.text == "buf-marker-warn"));
}

#[test]
fn cli_service_notices_buffer_in_structured_mode() {
    let output = OutputState::new();
    let _scope = scoped(&output);
    output.set_structured_output(true);
    let _ = output.drain_notices();

    CliService::warning("cli-marker-structured");
    let drained = output.drain_notices();
    output.set_structured_output(false);

    assert!(
        drained
            .iter()
            .any(|n| n.level == "warning" && n.text == "cli-marker-structured")
    );
}

#[test]
fn cli_service_notices_skip_buffer_when_not_structured() {
    let output = OutputState::new();
    let _scope = scoped(&output);
    output.set_structured_output(false);
    let _ = output.drain_notices();

    CliService::warning("cli-marker-plain");
    let drained = output.drain_notices();

    assert!(!drained.iter().any(|n| n.text == "cli-marker-plain"));
}

#[test]
fn mark_sets_structured_emitted() {
    let output = OutputState::new();
    let _scope = scoped(&output);
    output.mark_structured_emitted();
    assert!(output.structured_was_emitted());
}
