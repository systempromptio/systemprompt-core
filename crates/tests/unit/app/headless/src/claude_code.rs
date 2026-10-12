//! Claude Code adapter: argument golden, fixture decode and outcome mapping.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_headless::host::claude_code::ClaudeCode;
use systemprompt_headless::host::claude_code::args::claude_args;
use systemprompt_headless::host::claude_code::plugin::HostFiles;
use systemprompt_headless::{
    ErrorClass, ExitInfo, HeadlessHost, HookDecision, HostKind, McpServerSpec, PermissionMode,
    RunEventKind, RunEventPayload, RunRequest, RunStatus,
};
use systemprompt_identifiers::HostSessionId;

fn request() -> RunRequest {
    RunRequest {
        host: HostKind::ClaudeCode,
        prompt: "fix the bug".into(),
        system_prompt_append: Some("Be terse.".into()),
        persona: Some("You are a reviewer.".into()),
        model: Some("claude-sonnet-5-5".into()),
        allowed_tools: vec!["Read".into(), "Edit".into()],
        disallowed_tools: vec!["WebFetch".into()],
        permission_mode: PermissionMode::AcceptEdits,
        permission_prompts_none: false,
        workspace: PathBuf::from("/w"),
        add_dirs: vec![PathBuf::from("/extra")],
        mcp_servers: BTreeMap::new(),
        output_schema: None,
        max_turns: Some(5),
        max_budget_usd: Some(1.5),
        wall_clock: Duration::from_secs(60),
        resume_session_id: Some(HostSessionId::try_new("sess-9").expect("id")),
        attempt: 1,
        attribution_scopes: Vec::new(),
        plugin_id: None,
        plugin_dirs: vec![PathBuf::from("/plug")],
    }
}

#[test]
fn claude_args_golden() {
    let mut req = request();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    req.mcp_servers.insert(
        "s".into(),
        McpServerSpec::Http {
            url: "http://m".into(),
            headers: BTreeMap::new(),
        },
    );
    let files = HostFiles {
        mcp_config: Some(PathBuf::from("/run/host/mcp-config.json")),
        governance_plugin: Some(PathBuf::from("/run/host/governance-plugin")),
    };
    let expected: Vec<String> = [
        "-p",
        "fix the bug",
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        "claude-sonnet-5-5",
        "--permission-mode",
        "acceptEdits",
        "--allowedTools",
        "Read,Edit",
        "--disallowedTools",
        "WebFetch",
        "--append-system-prompt",
        "You are a reviewer.\n\nBe terse.",
        "--max-turns",
        "5",
        "--max-budget-usd",
        "1.50",
        "--add-dir",
        "/extra",
        "--mcp-config",
        "/run/host/mcp-config.json",
        "--strict-mcp-config",
        "--json-schema",
        "{\"type\":\"object\"}",
        "--resume",
        "sess-9",
        "--plugin-dir",
        "/plug",
        "--plugin-dir",
        "/run/host/governance-plugin",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(claude_args(&req, &files), expected);
}

#[test]
fn claude_args_skip_permissions_only_when_bypass_and_no_prompts() {
    let mut req = request();
    req.permission_mode = PermissionMode::BypassPermissions;
    req.permission_prompts_none = true;
    let args = claude_args(&req, &HostFiles::default());
    assert!(args.contains(&"--dangerously-skip-permissions".to_owned()));
    assert!(!args.contains(&"--permission-mode".to_owned()));
    req.permission_prompts_none = false;
    let args = claude_args(&req, &HostFiles::default());
    assert!(!args.contains(&"--dangerously-skip-permissions".to_owned()));
}

fn decode_fixture() -> (Vec<RunEventPayload>, Vec<HostSessionId>) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/claude-stream-sample.jsonl"
    );
    let text = std::fs::read_to_string(path).expect("fixture");
    let host = ClaudeCode;
    let events: Vec<_> = text
        .lines()
        .flat_map(|line| host.parse_line(line))
        .collect();
    let sessions = events.iter().filter_map(|e| e.session_id.clone()).collect();
    let payloads = events.into_iter().flat_map(|e| e.payloads).collect();
    (payloads, sessions)
}

#[test]
fn claude_code_decodes_fixture_in_order() {
    let (payloads, sessions) = decode_fixture();
    let kinds: Vec<RunEventKind> = payloads.iter().map(RunEventPayload::kind).collect();
    assert_eq!(
        kinds,
        vec![
            RunEventKind::Hook,
            RunEventKind::Init,
            RunEventKind::AssistantText,
            RunEventKind::ToolUse,
            RunEventKind::ToolResult,
            RunEventKind::AssistantText,
            RunEventKind::Result,
        ]
    );
    let expected = HostSessionId::try_new("c09fe04e-19d8-4776-b09a-be25d0f53c90").expect("id");
    assert!(!sessions.is_empty());
    assert!(sessions.iter().all(|s| *s == expected));
    assert!(matches!(
        &payloads[0],
        RunEventPayload::Hook {
            decision: HookDecision::Allow,
            ..
        }
    ));
    let RunEventPayload::Result {
        cost_microdollars,
        input_tokens,
        num_turns,
        ..
    } = &payloads[6]
    else {
        panic!("last payload is not a result");
    };
    assert_eq!(*cost_microdollars, 397_071);
    assert_eq!(*input_tokens, 34 + 18_948 + 41_085);
    assert_eq!(*num_turns, 2);
}

#[test]
fn claude_code_garbage_line_warns() {
    let events = ClaudeCode.parse_line("not json");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payloads[0].kind(), RunEventKind::Runner);
    assert!(
        ClaudeCode
            .parse_line("{\"type\":\"rate_limit_event\"}")
            .is_empty()
    );
}

fn exit(code: Option<i32>) -> ExitInfo {
    ExitInfo {
        code,
        timed_out: false,
        cancelled: false,
        duration_ms: 1234,
    }
}

fn result(subtype: &str, is_error: bool) -> RunEventPayload {
    RunEventPayload::Result {
        subtype: subtype.into(),
        is_error,
        num_turns: 3,
        duration_ms: 900,
        input_tokens: 10,
        output_tokens: 20,
        cost_microdollars: 5_000,
        structured_output: None,
        error: None,
    }
}

#[test]
fn claude_code_outcome_success_copies_usage() {
    let (payloads, _) = decode_fixture();
    let out = ClaudeCode.outcome(exit(Some(0)), &payloads, &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Succeeded, ErrorClass::Ok)
    );
    assert_eq!(out.turns, 2);
    assert_eq!(out.cost_microdollars, 397_071);
    assert!(out.session_id.is_some());
}

#[test]
fn claude_code_outcome_budget_limits() {
    for subtype in ["error_max_turns", "error_max_budget_usd"] {
        let out = ClaudeCode.outcome(exit(Some(1)), &[result(subtype, true)], &request());
        assert_eq!(
            (out.status, out.error_class),
            (RunStatus::Failed, ErrorClass::BudgetExceeded)
        );
        assert_eq!(out.turns, 3);
    }
}

#[test]
fn claude_code_outcome_hook_deny() {
    let deny = RunEventPayload::Hook {
        name: "PreToolUse".into(),
        decision: HookDecision::Deny,
        policy: Some("blocklist".into()),
    };
    let out = ClaudeCode.outcome(exit(Some(0)), &[deny, result("success", false)], &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::HookDenied)
    );
}

#[test]
fn claude_code_outcome_crash_timeout_cancel_schema() {
    let out = ClaudeCode.outcome(exit(Some(137)), &[], &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::HostCrashed)
    );
    assert_eq!(out.duration_ms, 1234);
    let timed = ExitInfo {
        timed_out: true,
        ..exit(None)
    };
    let out = ClaudeCode.outcome(timed, &[], &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::TimedOut, ErrorClass::Timeout)
    );
    let cancelled = ExitInfo {
        cancelled: true,
        ..exit(None)
    };
    let out = ClaudeCode.outcome(cancelled, &[], &request());
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Cancelled, ErrorClass::Ok)
    );
    let mut req = request();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    let out = ClaudeCode.outcome(exit(Some(0)), &[result("success", false)], &req);
    assert_eq!(
        (out.status, out.error_class),
        (RunStatus::Failed, ErrorClass::OutputInvalid)
    );
}
