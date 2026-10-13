//! Codex CLI adapter: outcome classification over the live and documented
//! fixtures, and defensive line decoding.

use systemprompt_agent_runs::host::codex_cli::CodexCli;
use systemprompt_agent_runs::{
    ErrorClass, ExitInfo, HeadlessHost, RunEventKind, RunEventPayload, RunStatus,
};

use crate::codex_cli::{LIVE_401, SAMPLE, decode, exit, request};

#[test]
fn codex_sample_outcome_succeeds_with_structured_output() {
    let mut req = request();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    let events = decode(SAMPLE);
    let out = CodexCli::default().outcome(exit(0), &events, &req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Succeeded, ErrorClass::Ok)
    );
    assert_eq!(
        (out.input_tokens, out.output_tokens, out.turns),
        (2140, 38, 1)
    );
    assert_eq!(out.structured_output, Some(serde_json::json!({"count": 2})));
    let final_text = out.final_text.as_deref().expect("final agent message");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(final_text.trim()).expect("json answer"),
        serde_json::json!({"count": 2})
    );
    assert_eq!(
        out.session_id.map(|s| s.as_str().to_owned()).as_deref(),
        Some("01a130c4-47d1-7c02-8e70-20ab9d91b57c")
    );
}

#[test]
fn codex_live_401_is_connector_unavailable() {
    let events = decode(LIVE_401);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.kind() == RunEventKind::Result)
            .count(),
        1
    );
    let out = CodexCli::default().outcome(exit(1), &events, &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::ConnectorUnavailable)
    );
    assert!(out.error.expect("error").contains("401 Unauthorized"));
}

#[test]
fn codex_outcome_precedence() {
    let host = CodexCli::default();
    let events = decode(SAMPLE);
    let req = request();
    let cancelled = ExitInfo {
        cancelled: true,
        ..exit(137)
    };
    assert_eq!(
        host.outcome(cancelled, &events, &req).status,
        RunStatus::Cancelled
    );
    let timed = ExitInfo {
        timed_out: true,
        ..exit(137)
    };
    let out = host.outcome(timed, &events, &req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::TimedOut, ErrorClass::Timeout)
    );
    let out = host.outcome(exit(2), &[], &req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::HostCrashed)
    );
    let mut schema_req = request();
    schema_req.output_schema = Some(serde_json::json!({"type": "object"}));
    let prose: Vec<RunEventPayload> = decode(SAMPLE)
        .into_iter()
        .map(|e| match e {
            RunEventPayload::AssistantText { .. } => {
                RunEventPayload::AssistantText { text: "two".into() }
            },
            other => other,
        })
        .collect();
    let out = host.outcome(exit(0), &prose, &schema_req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::OutputInvalid)
    );
}

#[test]
fn codex_garbage_line_is_runner_warning() {
    let host = CodexCli::default();
    assert!(host.parse_line("").is_empty());
    assert!(host.parse_line(r#"{"type":"turn.started"}"#).is_empty());
    let events = host.parse_line("not json");
    assert_eq!(events[0].payloads[0].kind(), RunEventKind::Runner);
}
