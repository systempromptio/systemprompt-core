//! `OpenCode` V2 adapter: argument, environment and `opencode.json` goldens,
//! fixture decode and a `prepare` round-trip.
//!
//! Fixtures are live captures from `@opencode/cli` 2.1.3 against a stub
//! OpenAI-compatible provider: `opencode-run-sample.jsonl` (an invalid tool
//! call then a final text), `opencode-run-tool.jsonl` (a completed `shell`
//! call) and `opencode-run-no-route.jsonl` (a `provider.no-route` error).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use systemprompt_agent_runs::host::opencode::OpenCodeCli;
use systemprompt_agent_runs::host::opencode::args::{
    DISABLE_SWITCHES, OpenCodePaths, build_args, build_env,
};
use systemprompt_agent_runs::host::opencode::config::build;
use systemprompt_agent_runs::host::opencode::events::{MAX_EVENTS, MAX_LINE_BYTES};
use systemprompt_agent_runs::{
    Execution, ExitInfo, HeadlessHost, HostKind, McpServerSpec, PermissionMode, PrepareContext,
    RunCredentials, RunEventKind, RunEventPayload, RunLayout, RunRequest, RunnerLevel, host_for,
};
use systemprompt_identifiers::{
    ApiKeyId, ApiKeySecret, HostSessionId, ScopeDimension, SessionId, UserId,
};
use systemprompt_models::attribution::ScopeBinding;

pub(crate) const SAMPLE: &str = include_str!("../fixtures/opencode-run-sample.jsonl");
pub(crate) const TOOL: &str = include_str!("../fixtures/opencode-run-tool.jsonl");
pub(crate) const NO_ROUTE: &str = include_str!("../fixtures/opencode-run-no-route.jsonl");

pub(crate) fn request() -> RunRequest {
    RunRequest {
        host: HostKind::OpenCodeCli,
        prompt: "count the files".into(),
        system_prompt_append: Some("Be terse.".into()),
        persona: Some("You are a counter.".into()),
        model: Some("dyn-fast".into()),
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: PermissionMode::Default,
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
        harness: None,
        traffic: systemprompt_agent_runs::TrafficClass::Production,
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
        mcp_token: None,
        mcp_token_jti: None,
        mcp_token_expires_at: None,
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
    let host = OpenCodeCli::default();
    fixture
        .lines()
        .flat_map(|l| host.parse_line(l))
        .flat_map(|e| e.payloads)
        .collect()
}

#[test]
fn host_for_opencode_cli_is_supported() {
    let Execution::Supported(host) = host_for(HostKind::OpenCodeCli) else {
        panic!("unsupported")
    };
    let caps = host.capabilities();
    assert!(!caps.hooks && caps.resume && caps.structured_output && caps.mcp);
    assert_eq!(host.persistent_home_paths(), [".local/share/opencode"]);
    let requirement = host.version_requirement().expect("version gate");
    assert_eq!(requirement.args, ["--version"]);
    assert_eq!(requirement.minimum, semver::Version::new(2, 0, 30));
}

#[test]
fn opencode_args_golden() {
    let mut req = request();
    assert_eq!(
        build_args(&req),
        [
            "run",
            "--standalone",
            "--format",
            "json",
            "--auto",
            "-m",
            "systemprompt/dyn-fast",
            "--",
            "count the files",
        ]
    );
    req.model = None;
    req.resume_session_id = Some(HostSessionId::try_new("ses_9").expect("id"));
    assert_eq!(
        build_args(&req),
        [
            "run",
            "--standalone",
            "--format",
            "json",
            "--auto",
            "--session",
            "ses_9",
            "--",
            "count the files",
        ]
    );
}

#[test]
fn opencode_env_isolates_every_config_tier() {
    let paths = OpenCodePaths::new(&RunLayout::new("/r"));
    let env: BTreeMap<String, String> = build_env(&credentials(), &paths).into_iter().collect();
    assert_eq!(env["HOME"], "/r/home");
    assert_eq!(env["XDG_CONFIG_HOME"], "/r/home/.config");
    assert_eq!(env["XDG_DATA_HOME"], "/r/home/.local/share");
    assert_eq!(env["XDG_STATE_HOME"], "/r/home/.local/state");
    assert_eq!(env["XDG_CACHE_HOME"], "/r/home/.cache");
    assert_eq!(env["SYSTEMPROMPT_PAT"], "sp-pat");
    assert_eq!(env["DO_NOT_TRACK"], "1");
    for key in DISABLE_SWITCHES {
        assert_eq!(env[key], "1", "{key}");
    }
    assert_eq!(
        paths.config_json,
        PathBuf::from("/r/home/.config/opencode/opencode.json")
    );
}

#[test]
fn opencode_config_golden() {
    let mut req = request();
    req.allowed_tools = vec!["read".into(), "edit".into()];
    req.disallowed_tools = vec!["webfetch".into()];
    req.attribution_scopes = vec![ScopeBinding {
        dimension: ScopeDimension::try_new("department").expect("dim"),
        value: "sales".into(),
    }];
    req.mcp_servers.insert(
        "odoo".into(),
        McpServerSpec::Http {
            url: "https://gw/mcp/odoo".into(),
            headers: BTreeMap::new(),
        },
    );
    let json = serde_json::to_value(build(&req, &credentials(), "https://gw/")).expect("json");
    assert_eq!(
        json,
        serde_json::json!({
            "$schema": "https://opencode.ai/config.json",
            "model": "systemprompt/dyn-fast",
            "update": "disable",
            "share": "disabled",
            "providers": {
                "systemprompt": {
                    "name": "systemprompt",
                    "package": "aisdk:@ai-sdk/openai-compatible",
                    "settings": {
                        "baseURL": "https://gw/v1",
                        "apiKey": "{env:SYSTEMPROMPT_PAT}",
                        "headers": {
                            "x-session-id": "sess-1",
                            "x-systemprompt-client": "headless-opencode",
                            "x-systemprompt-scope-department": "sales"
                        }
                    },
                    "models": { "dyn-fast": { "modelID": "dyn-fast" } }
                }
            },
            "policies": [
                { "effect": "deny", "action": "provider.use", "resource": "*" },
                { "effect": "allow", "action": "provider.use", "resource": "systemprompt" },
                { "effect": "deny", "action": "integration.use", "resource": "mcp:*" },
                { "effect": "deny", "action": "integration.use", "resource": "plugin:*" },
                { "effect": "allow", "action": "integration.use", "resource": "mcp:odoo" },
                { "effect": "deny", "action": "tool.use", "resource": "*" },
                { "effect": "allow", "action": "tool.use", "resource": "read:*" },
                { "effect": "allow", "action": "tool.use", "resource": "edit:*" },
                { "effect": "deny", "action": "tool.use", "resource": "webfetch:*" }
            ],
            "mcp": { "servers": { "odoo": { "type": "remote", "url": "https://gw/mcp/odoo", "oauth": false } } }
        })
    );
}

#[test]
fn opencode_plan_mode_denies_mutating_tools_last() {
    let mut req = request();
    req.permission_mode = PermissionMode::Plan;
    let json = serde_json::to_value(build(&req, &credentials(), "https://gw")).expect("json");
    let tail: Vec<&str> = json["policies"]
        .as_array()
        .expect("policies")
        .iter()
        .rev()
        .take(3)
        .map(|p| p["resource"].as_str().expect("resource"))
        .collect();
    assert_eq!(tail, ["shell:*", "write:*", "edit:*"]);
}

#[test]
fn opencode_sample_decodes_init_tools_and_text() {
    let events = decode(SAMPLE);
    let kinds: Vec<RunEventKind> = events.iter().map(RunEventPayload::kind).collect();
    assert_eq!(
        kinds,
        [
            RunEventKind::Init,
            RunEventKind::ToolUse,
            RunEventKind::ToolResult,
            RunEventKind::Result,
            RunEventKind::AssistantText,
        ]
    );
    let RunEventPayload::ToolResult { is_error, .. } = &events[2] else {
        panic!("tool result")
    };
    assert!(*is_error, "invalid tool arguments are an errored result");
}

#[test]
fn opencode_completed_tool_is_one_use_and_one_result() {
    let host = OpenCodeCli::default();
    let mut lines: Vec<&str> = TOOL.lines().collect();
    lines.insert(2, lines[1]);
    let events: Vec<RunEventPayload> = lines
        .iter()
        .flat_map(|l| host.parse_line(l))
        .flat_map(|e| e.payloads)
        .collect();
    let uses: Vec<&RunEventPayload> = events
        .iter()
        .filter(|e| e.kind() == RunEventKind::ToolUse)
        .collect();
    assert_eq!(uses.len(), 1, "a repeated part id is deduped");
    let RunEventPayload::ToolUse { name, input, .. } = uses[0] else {
        panic!("tool use")
    };
    assert_eq!(name, "shell");
    assert_eq!(input["command"], "echo PWNED-$((6*7))");
    assert!(events.iter().any(|e| matches!(
        e,
        RunEventPayload::ToolResult { content, is_error: false, .. } if content == "PWNED-42\n"
    )));
}

#[test]
fn opencode_mixed_session_is_refused() {
    let host = OpenCodeCli::default();
    let first = SAMPLE.lines().next().expect("line");
    let other = first.replace("ses_e98161a61ffeCsq3SWs9aGRv9x", "ses_other");
    let _ = host.parse_line(first);
    let refused: Vec<RunEventPayload> = host
        .parse_line(&other)
        .into_iter()
        .flat_map(|e| e.payloads)
        .collect();
    assert!(matches!(
        refused.as_slice(),
        [RunEventPayload::Runner { level: RunnerLevel::Error, message }] if message.contains("switched session")
    ));
}

#[test]
fn opencode_decoder_is_bounded() {
    let host = OpenCodeCli::default();
    let long = format!(
        "{{\"type\":\"text\",\"x\":\"{}\"}}",
        "a".repeat(MAX_LINE_BYTES)
    );
    assert!(matches!(
        host.parse_line(&long)[0].payloads.as_slice(),
        [RunEventPayload::Runner {
            level: RunnerLevel::Warn,
            ..
        }]
    ));
    let line = r#"{"type":"step_start"}"#;
    for _ in 1..MAX_EVENTS {
        let _ = host.parse_line(line);
    }
    let over = host.parse_line(line);
    assert!(matches!(
        over[0].payloads.as_slice(),
        [RunEventPayload::Runner {
            level: RunnerLevel::Error,
            ..
        }]
    ));
    assert!(
        host.parse_line(SAMPLE.lines().last().expect("line"))
            .is_empty()
    );
}

#[test]
fn opencode_non_json_line_is_a_warning() {
    let events: Vec<RunEventPayload> = OpenCodeCli::default()
        .parse_line("Error: something")
        .into_iter()
        .flat_map(|e| e.payloads)
        .collect();
    assert!(matches!(
        events.as_slice(),
        [RunEventPayload::Runner {
            level: RunnerLevel::Warn,
            ..
        }]
    ));
}

#[test]
fn opencode_prepare_writes_config_and_agents_md() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    let mut req = request();
    req.workspace = dir.path().join("workspace");
    let creds = credentials();
    let host = OpenCodeCli::default();
    let invocation = host
        .prepare(&PrepareContext {
            request: &req,
            layout: &layout,
            credentials: &creds,
            gateway_base_url: "https://gw",
            hooks_file: None,
        })
        .expect("prepare");
    assert_eq!(invocation.program, "opencode");
    assert_eq!(invocation.cwd, req.workspace);
    assert!(invocation.stdin.is_none());
    let paths = OpenCodePaths::new(&layout);
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&paths.config_json).expect("config"))
            .expect("json");
    assert_eq!(
        written["providers"]["systemprompt"]["settings"]["apiKey"],
        "{env:SYSTEMPROMPT_PAT}"
    );
    assert_eq!(
        std::fs::read_to_string(&paths.agents_md).expect("agents"),
        "You are a counter.\n\nBe terse.\n"
    );
}
