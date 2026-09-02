//! Unit tests for the streaming-tap finalize decision.

use systemprompt_api::services::gateway::stream_tap::{FailCause, FinalizeDecision, classify};

#[test]
fn empty_stream_fails_not_completes() {
    assert_eq!(
        classify(None, false, false, false),
        FinalizeDecision::Fail(FailCause::Truncated { has_content: false }),
    );
}

#[test]
fn truncated_stream_with_content_but_no_stop_fails() {
    assert_eq!(
        classify(None, false, true, false),
        FinalizeDecision::Fail(FailCause::Truncated { has_content: true }),
    );
}

#[test]
fn upstream_error_always_fails() {
    assert_eq!(
        classify(Some("boom"), true, true, true),
        FinalizeDecision::Fail(FailCause::Upstream),
    );
}

#[test]
fn normal_stream_completes_without_capture_miss() {
    assert_eq!(
        classify(None, true, true, true),
        FinalizeDecision::Complete {
            cost_capture_miss: false
        },
    );
}

#[test]
fn served_but_unmetered_stream_completes_with_capture_miss() {
    assert_eq!(
        classify(None, true, true, false),
        FinalizeDecision::Complete {
            cost_capture_miss: true
        },
    );
}

#[test]
fn stop_without_content_is_not_a_capture_miss() {
    assert_eq!(
        classify(None, true, false, false),
        FinalizeDecision::Complete {
            cost_capture_miss: false
        },
    );
}

#[test]
fn fail_causes_carry_distinct_reasons() {
    assert_eq!(FailCause::Upstream.reason(), "upstream stream error");
    assert_eq!(
        FailCause::Truncated { has_content: true }.reason(),
        "stream ended without stop event",
    );
    assert_eq!(
        FailCause::Truncated { has_content: false }.reason(),
        "empty upstream stream",
    );
}
