//! Codex CLI adapter: argument and `config.toml` goldens, fixture decode,
//! outcome mapping and a `prepare` round-trip.
//!
//! Fixtures: `codex-exec-401.jsonl` is a trimmed live capture from Codex CLI
//! 0.157.0 run with no credentials (thread start, retry errors, the error
//! item, `turn.failed`). `codex-exec-sample.jsonl` is the success path; it
//! could not be captured live (no reachable provider), so it is written from
//! the upstream `codex-rs/exec/src/exec_events.rs` schema, reusing the live
//! thread id.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_headless::host::codex_cli::CodexCli;
use systemprompt_headless::host::codex_cli::args::{CodexPaths, agents_md, build_args, build_env};
use systemprompt_headless::host::codex_cli::config_toml::{quote, render, sandbox_mode};
use systemprompt_headless::{
    Execution, ExitInfo, HeadlessHost, HostKind, McpServerSpec, PermissionMode, PrepareContext,
    RunCredentials, RunEventKind, RunEventPayload, RunLayout, RunRequest, host_for,
};
use systemprompt_identifiers::{
    ApiKeyId, ApiKeySecret, HostSessionId, ScopeDimension, SessionId, UserId,
};
use systemprompt_models::attribution::ScopeBinding;

pub(crate) const SAMPLE: &str = include_str!("../fixtures/codex-exec-sample.jsonl");
pub(crate) const LIVE_401: &str = include_str!("../fixtures/codex-exec-401.jsonl");

pub(crate) fn request() -> RunRequest {
    RunRequest {
        host: HostKind::CodexCli,
        prompt: "count the files".into(),
        system_prompt_append: Some("Be terse.".into()),
        persona: Some("You are a counter.".into()),
        model: Some("gpt-5".into()),
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: PermissionMode::AcceptEdits,
        permission_prompts_none: true,
        workspace: PathBuf::from("/r/workspace"),
        add_dirs: Vec::new(),
        mcp_servers: BTreeMap::new(),
        output_schema: None,
        max_turns: None,
        max_budget_usd: None,
        wall_clock: Duration::from_secs(60),
        resume_session_id: None,
        attempt: 1,
        attribution_scopes: Vec::new(),
        plugin_id: None,
        plugin_dirs: Vec::new(),
    }
}

pub(crate) fn credentials() -> RunCredentials {
    RunCredentials {
        user_id: UserId::new("user-1"),
        api_key_id: ApiKeyId::new("key-1"),
        pat: ApiKeySecret::new("sp-pat"),
        gateway_session_id: SessionId::new("sess-1"),
        hook_token: None,
        hook_token_jti: None,
        hook_token_expires_at: None,
    }
}

pub(crate) fn exit(code: i32) -> ExitInfo {
    ExitInfo {
        code: Some(code),
        timed_out: false,
        cancelled: false,
        duration_ms: 900,
    }
}

pub(crate) fn decode(fixture: &str) -> Vec<RunEventPayload> {
    let host = CodexCli::default();
    fixture
        .lines()
        .flat_map(|l| host.parse_line(l))
        .flat_map(|e| e.payloads)
        .collect()
}

#[test]
fn host_for_codex_is_supported() {
    let Execution::Supported(host) = host_for(HostKind::CodexCli) else {
        panic!("unsupported")
    };
    let caps = host.capabilities();
    assert!(!caps.hooks && caps.resume && caps.structured_output && caps.mcp);
    assert!(matches!(
        host_for(HostKind::OpenCode),
        Execution::Unsupported { .. }
    ));
    assert!(matches!(
        host_for(HostKind::Hermes),
        Execution::Unsupported { .. }
    ));
}

#[test]
fn codex_args_golden() {
    let paths = CodexPaths::new(&RunLayout::new("/r"));
    let mut req = request();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    assert_eq!(
        build_args(&req, &paths),
        [
            "exec",
            "--json",
            "--skip-git-repo-check",
            "--cd",
            "/r/workspace",
            "--model",
            "gpt-5",
            "--output-schema",
            "/r/host/output-schema.json",
            "--output-last-message",
            "/r/logs/last-message.txt",
            "-",
        ]
    );
    req.resume_session_id = Some(HostSessionId::try_new("thread-9").expect("id"));
    req.output_schema = None;
    req.model = None;
    assert_eq!(
        build_args(&req, &paths),
        [
            "exec",
            "resume",
            "--json",
            "--skip-git-repo-check",
            "--output-last-message",
            "/r/logs/last-message.txt",
            "thread-9",
            "-",
        ]
    );
    assert_eq!(
        build_env(&credentials(), &paths),
        [
            ("HOME".to_owned(), "/r/home".to_owned()),
            ("CODEX_HOME".to_owned(), "/r/home/.codex".to_owned()),
            ("SYSTEMPROMPT_PAT".to_owned(), "sp-pat".to_owned()),
        ]
    );
    assert_eq!(
        agents_md(&req).as_deref(),
        Some("You are a counter.\n\nBe terse.\n")
    );
}

#[test]
fn codex_config_toml_golden() {
    let mut req = request();
    req.add_dirs = vec![PathBuf::from("/extra")];
    req.mcp_servers.insert(
        "fs".into(),
        McpServerSpec::Stdio {
            command: "fs-mcp".into(),
            args: vec!["--root".into(), "/r".into()],
            env: BTreeMap::from([("K".into(), "v\"q".into())]),
        },
    );
    req.mcp_servers.insert(
        "web".into(),
        McpServerSpec::Http {
            url: "http://m/mcp".into(),
            headers: BTreeMap::new(),
        },
    );
    req.attribution_scopes = vec![ScopeBinding {
        dimension: ScopeDimension::try_new("workflow").expect("dimension"),
        value: "run-1".into(),
    }];
    let expected = r#"model_provider = "systemprompt"
approval_policy = "never"
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = ["/extra"]

[model_providers.systemprompt]
name = "systemprompt"
base_url = "https://gw.example/v1"
wire_api = "responses"
env_key = "SYSTEMPROMPT_PAT"
http_headers = { "x-session-id" = "sess-1", "x-systemprompt-client" = "headless-codex", "x-systemprompt-scope-workflow" = "run-1" }

[mcp_servers."fs"]
command = "fs-mcp"
args = ["--root", "/r"]
env = { "K" = "v\"q" }

[mcp_servers."web"]
url = "http://m/mcp"
"#;
    assert_eq!(
        render(&req, &credentials(), "https://gw.example/"),
        expected
    );
    assert_eq!(sandbox_mode(PermissionMode::Plan), "read-only");
    assert_eq!(
        sandbox_mode(PermissionMode::BypassPermissions),
        "danger-full-access"
    );
    assert_eq!(quote("a\\b\n"), r#""a\\b\n""#);
}

#[test]
fn codex_sample_fixture_decodes() {
    let events = decode(SAMPLE);
    let kinds: Vec<RunEventKind> = events.iter().map(RunEventPayload::kind).collect();
    use RunEventKind::*;
    assert_eq!(
        kinds,
        [
            Init,
            ToolUse,
            ToolResult,
            ToolUse,
            ToolResult,
            ToolUse,
            ToolResult,
            AssistantText,
            Result
        ]
    );
    let RunEventPayload::Init { session_id, .. } = &events[0] else {
        panic!()
    };
    assert_eq!(session_id.as_str(), "01a130c4-47d1-7c02-8e70-20ab9d91b57c");
    let RunEventPayload::ToolUse { name, input, .. } = &events[3] else {
        panic!()
    };
    assert_eq!(name, "mcp__fs__stat");
    assert_eq!(input, &serde_json::json!({"path": "a"}));
    let RunEventPayload::ToolResult {
        tool_use_id,
        content,
        is_error,
    } = &events[2]
    else {
        panic!()
    };
    assert_eq!(
        (tool_use_id.as_str(), content.as_str(), *is_error),
        ("item_1", "a\nb\n", false)
    );
    let RunEventPayload::ToolUse { name, .. } = &events[5] else {
        panic!()
    };
    assert_eq!(name, "file_change");
}

#[test]
fn codex_prepare_writes_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    let mut req = request();
    req.workspace = layout.workspace.clone();
    req.output_schema = Some(serde_json::json!({"type": "object"}));
    let creds = credentials();
    let ctx = PrepareContext {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://gw",
        hooks_file: None,
    };
    let inv = CodexCli::default().prepare(&ctx).expect("prepare");
    assert_eq!(inv.program, "codex");
    assert_eq!(inv.stdin.as_deref(), Some("count the files"));
    assert_eq!(inv.cwd, layout.workspace);
    let toml = std::fs::read_to_string(layout.home.join(".codex/config.toml")).expect("toml");
    assert!(toml.contains(r#"base_url = "http://gw/v1""#));
    assert!(layout.workspace.join("AGENTS.md").exists());
    assert!(layout.host.join("output-schema.json").exists());
}
