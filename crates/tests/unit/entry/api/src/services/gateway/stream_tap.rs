//! Unit tests for the streaming-tap finalize decision.

use systemprompt_api::services::gateway::stream_tap::{
    ClientConnection, FailCause, FinalizeDecision, classify,
};

#[test]
fn empty_stream_fails_not_completes() {
    assert_eq!(
        classify(None, false, false, false, ClientConnection::Connected),
        FinalizeDecision::Fail(FailCause::Truncated {
            has_content: false,
            client_gone: false,
        }),
    );
}

#[test]
fn truncated_stream_with_content_but_no_stop_fails() {
    assert_eq!(
        classify(None, false, true, false, ClientConnection::Connected),
        FinalizeDecision::Fail(FailCause::Truncated {
            has_content: true,
            client_gone: false,
        }),
    );
}

#[test]
fn upstream_error_always_fails() {
    assert_eq!(
        classify(Some("boom"), true, true, true, ClientConnection::Connected),
        FinalizeDecision::Fail(FailCause::Upstream),
    );
}

#[test]
fn normal_stream_completes_without_capture_miss() {
    assert_eq!(
        classify(None, true, true, true, ClientConnection::Connected),
        FinalizeDecision::Complete {
            cost_capture_miss: false
        },
    );
}

#[test]
fn served_but_unmetered_stream_completes_with_capture_miss() {
    assert_eq!(
        classify(None, true, true, false, ClientConnection::Connected),
        FinalizeDecision::Complete {
            cost_capture_miss: true
        },
    );
}

#[test]
fn stop_without_content_is_not_a_capture_miss() {
    assert_eq!(
        classify(None, true, false, false, ClientConnection::Connected),
        FinalizeDecision::Complete {
            cost_capture_miss: false
        },
    );
}

#[test]
fn client_drop_before_stop_is_client_gone() {
    assert_eq!(
        classify(None, false, true, false, ClientConnection::Disconnected),
        FinalizeDecision::Fail(FailCause::Truncated {
            has_content: true,
            client_gone: true,
        }),
    );
}

#[test]
fn upstream_error_outranks_client_drop() {
    assert_eq!(
        classify(
            Some("boom"),
            false,
            true,
            false,
            ClientConnection::Disconnected
        ),
        FinalizeDecision::Fail(FailCause::Upstream),
    );
}

#[test]
fn fail_causes_carry_distinct_reasons() {
    assert_eq!(FailCause::Upstream.reason(), "upstream stream error");
    assert_eq!(
        FailCause::Truncated {
            has_content: true,
            client_gone: false,
        }
        .reason(),
        "upstream stream ended without stop event",
    );
    assert_eq!(
        FailCause::Truncated {
            has_content: false,
            client_gone: false,
        }
        .reason(),
        "empty upstream stream",
    );
}

#[test]
fn client_gone_reason_never_blames_upstream() {
    for has_content in [true, false] {
        assert_eq!(
            FailCause::Truncated {
                has_content,
                client_gone: true,
            }
            .reason(),
            "client disconnected before stop event",
        );
    }
}
