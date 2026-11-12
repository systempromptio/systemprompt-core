//! Declared tools restrict the run: Claude Code `--tools`, Codex refusal.

use std::collections::BTreeMap;

use systemprompt_agent_runs::host::claude_code::args::claude_args;
use systemprompt_agent_runs::host::claude_code::plugin::HostFiles;
use systemprompt_agent_runs::host::codex_cli::CodexCli;
use systemprompt_agent_runs::{
    AgentRunsError, HeadlessHost, HostKind, McpServerSpec, PermissionMode, PrepareContext,
    RunLayout, RunRequest,
};

fn claude_request() -> RunRequest {
    let mut req = crate::codex_cli::request();
    req.host = HostKind::ClaudeCode;
    req
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn http_server() -> McpServerSpec {
    McpServerSpec::Http {
        url: "http://m".into(),
        headers: BTreeMap::new(),
    }
}

fn refused_entry(result: Result<Vec<String>, AgentRunsError>) -> String {
    match result {
        Err(AgentRunsError::ToolPolicyUnenforceable { host, entry, .. }) => {
            assert_eq!(host, HostKind::ClaudeCode);
            entry
        },
        other => panic!("expected tool policy refusal, got {other:?}"),
    }
}

#[test]
fn declared_tools_restrict_the_builtin_set_by_base_name() {
    let mut req = claude_request();
    req.allowed_tools = vec!["Read".into(), "Bash(git:*)".into(), "Bash(ls)".into()];
    let args = claude_args(&req, &HostFiles::default()).expect("args");
    assert_eq!(flag_value(&args, "--tools").as_deref(), Some("Read,Bash"));
    assert_eq!(
        flag_value(&args, "--allowedTools").as_deref(),
        Some("Read,Bash(git:*),Bash(ls)")
    );
}

#[test]
fn empty_declared_tools_leave_the_tool_set_alone() {
    let args = claude_args(&claude_request(), &HostFiles::default()).expect("args");
    assert!(!args.contains(&"--tools".to_owned()));
    assert!(!args.contains(&"--allowedTools".to_owned()));
}

#[test]
fn only_mcp_servers_declared_disables_every_builtin() {
    let mut req = claude_request();
    req.mcp_servers.insert("fs".into(), http_server());
    req.allowed_tools = vec!["mcp__fs__*".into()];
    let args = claude_args(&req, &HostFiles::default()).expect("args");
    assert_eq!(flag_value(&args, "--tools").as_deref(), Some(""));
}

#[test]
fn a_single_mcp_tool_is_refused_under_bypass() {
    let mut req = claude_request();
    req.permission_mode = PermissionMode::BypassPermissions;
    req.mcp_servers.insert("fs".into(), http_server());
    req.allowed_tools = vec!["mcp__fs__stat".into()];
    assert_eq!(
        refused_entry(claude_args(&req, &HostFiles::default())),
        "mcp__fs__stat"
    );
}

#[test]
fn an_undeclared_mcp_server_is_refused_under_bypass() {
    let mut req = claude_request();
    req.permission_mode = PermissionMode::BypassPermissions;
    req.mcp_servers.insert("fs".into(), http_server());
    req.allowed_tools = vec!["Read".into()];
    assert_eq!(
        refused_entry(claude_args(&req, &HostFiles::default())),
        "fs"
    );
}

#[test]
fn a_single_mcp_tool_is_preapproved_outside_bypass() {
    let mut req = claude_request();
    req.mcp_servers.insert("fs".into(), http_server());
    req.mcp_servers.insert("other".into(), http_server());
    req.allowed_tools = vec!["Read".into(), "mcp__fs__stat".into()];
    let args = claude_args(&req, &HostFiles::default()).expect("args");
    assert_eq!(flag_value(&args, "--tools").as_deref(), Some("Read"));
    assert_eq!(
        flag_value(&args, "--allowedTools").as_deref(),
        Some("Read,mcp__fs__stat")
    );
}

#[test]
fn an_unconfigured_mcp_server_is_refused_in_every_mode() {
    let mut req = claude_request();
    req.allowed_tools = vec!["mcp__ghost__stat".into()];
    assert_eq!(
        refused_entry(claude_args(&req, &HostFiles::default())),
        "mcp__ghost__stat"
    );
}

#[test]
fn a_malformed_entry_is_refused() {
    let mut req = claude_request();
    req.allowed_tools = vec!["(git:*)".into()];
    assert_eq!(
        refused_entry(claude_args(&req, &HostFiles::default())),
        "(git:*)"
    );
}

fn codex_prepare(req: &RunRequest) -> Result<(), AgentRunsError> {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    let mut req = req.clone();
    req.workspace = layout.workspace.clone();
    let creds = crate::codex_cli::credentials();
    let ctx = PrepareContext {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://gw",
        hooks_file: None,
    };
    let result = CodexCli::default().prepare(&ctx).map(|_| ());
    assert!(
        result.is_ok() || !layout.home.join(".codex/config.toml").exists(),
        "a refused codex run wrote its config"
    );
    result
}

#[test]
fn codex_refuses_declared_tools() {
    let mut req = crate::codex_cli::request();
    req.allowed_tools = vec!["shell".into()];
    match codex_prepare(&req) {
        Err(AgentRunsError::ToolPolicyUnenforceable { host, entry, .. }) => {
            assert_eq!(host, HostKind::CodexCli);
            assert_eq!(entry, "shell");
        },
        other => panic!("expected tool policy refusal, got {other:?}"),
    }
}

#[test]
fn codex_refuses_disallowed_tools() {
    let mut req = crate::codex_cli::request();
    req.disallowed_tools = vec!["web_search".into()];
    assert!(matches!(
        codex_prepare(&req),
        Err(AgentRunsError::ToolPolicyUnenforceable { .. })
    ));
}

#[test]
fn codex_without_tool_lists_prepares() {
    codex_prepare(&crate::codex_cli::request()).expect("prepare");
}
