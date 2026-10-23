//! `OpenCode` V2 adapter: outcome classification from events, never the
//! exit code.

use systemprompt_agent_runs::host::opencode::OpenCodeCli;
use systemprompt_agent_runs::{ErrorClass, ExitInfo, HeadlessHost, RunStatus};

use crate::opencode::{NO_ROUTE, SAMPLE, TOOL, decode, exit, request};

#[test]
fn opencode_sample_succeeds_with_tokens_from_steps() {
    let out = OpenCodeCli::default().outcome(exit(0), &decode(SAMPLE), &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Succeeded, ErrorClass::Ok)
    );
    assert_eq!(
        (out.input_tokens, out.output_tokens, out.turns),
        (100, 10, 1)
    );
    assert_eq!(out.cost_microdollars, 0);
    assert_eq!(out.final_text.as_deref(), Some("DONE: final answer"));
    assert_eq!(
        out.session_id.map(|s| s.as_str().to_owned()).as_deref(),
        Some("ses_e98161a61ffeCsq3SWs9aGRv9x")
    );
}

#[test]
fn opencode_no_route_is_connector_unavailable_despite_exit_zero() {
    let out = OpenCodeCli::default().outcome(exit(0), &decode(NO_ROUTE), &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::ConnectorUnavailable)
    );
    assert!(out.error.expect("error").contains("provider.no-route"));
}

#[test]
fn opencode_other_error_is_host_crashed() {
    let line = r#"{"type":"error","sessionID":"ses_1","error":{"type":"session.aborted","message":"boom"}}"#;
    let out = OpenCodeCli::default().outcome(exit(0), &decode(line), &request());
    assert_eq!(out.error_class, ErrorClass::HostCrashed);
}

#[test]
fn opencode_empty_stream_is_host_crashed() {
    let out = OpenCodeCli::default().outcome(exit(0), &[], &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::HostCrashed)
    );
}

#[test]
fn opencode_outcome_precedence() {
    let host = OpenCodeCli::default();
    let events = decode(NO_ROUTE);
    let cancelled = ExitInfo {
        cancelled: true,
        ..exit(0)
    };
    assert_eq!(
        host.outcome(cancelled, &events, &request()).status,
        RunStatus::Cancelled
    );
    let timed_out = ExitInfo {
        timed_out: true,
        ..exit(0)
    };
    assert_eq!(
        host.outcome(timed_out, &events, &request()).error_class,
        ErrorClass::Timeout
    );
}

#[test]
fn opencode_output_schema_requires_json_final_text() {
    let mut req = request();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    let out = OpenCodeCli::default().outcome(exit(0), &decode(TOOL), &req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::OutputInvalid)
    );
    let json_text = SAMPLE.replace("DONE: final answer", r#"{\"count\": 2}"#);
    let out = OpenCodeCli::default().outcome(exit(0), &decode(&json_text), &req);
    assert_eq!(out.status, RunStatus::Succeeded);
    assert_eq!(out.structured_output, Some(serde_json::json!({"count": 2})));
}
