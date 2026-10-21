use std::collections::BTreeMap;
use std::path::PathBuf;

use systemprompt_agent_runs::host::claude_code::ClaudeCode;
use systemprompt_agent_runs::{
    AgentRunsError, Execution, HeadlessHost, HostKind, McpServerSpec, PrepareContext, RunLayout,
    RunRequest, host_for,
};

fn claude_request() -> RunRequest {
    let mut req = crate::codex_cli::request();
    req.host = HostKind::ClaudeCode;
    req
}

fn servers() -> BTreeMap<String, McpServerSpec> {
    let mut servers = BTreeMap::new();
    servers.insert(
        "local".to_owned(),
        McpServerSpec::Stdio {
            command: "mcp-local".to_owned(),
            args: vec!["--fast".to_owned()],
            env: BTreeMap::from([("K".to_owned(), "V".to_owned())]),
        },
    );
    servers.insert(
        "remote".to_owned(),
        McpServerSpec::Http {
            url: "https://mcp.example.test/mcp".to_owned(),
            headers: BTreeMap::from([("Authorization".to_owned(), "Bearer t".to_owned())]),
        },
    );
    servers
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

#[test]
fn claude_code_is_a_supported_host_with_full_capabilities() {
    let Execution::Supported(host) = host_for(HostKind::ClaudeCode) else {
        panic!("claude code must be supported");
    };
    assert_eq!(host.kind(), HostKind::ClaudeCode);
    let caps = host.capabilities();
    assert!(caps.hooks && caps.resume && caps.structured_output && caps.mcp);
    assert_eq!(host.persistent_home_paths(), &[".claude/projects"]);
}

#[test]
fn prepare_writes_the_mcp_config_and_loads_the_governance_plugin() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    let mut req = claude_request();
    req.mcp_servers = servers();
    let creds = crate::codex_cli::credentials();
    let plugin_dir = dir.path().join("gov-plugin");
    let ctx = PrepareContext {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://gw",
        hooks_file: Some(plugin_dir.clone()),
    };

    let inv = ClaudeCode.prepare(&ctx).expect("prepare");

    assert_eq!(inv.program, "claude");
    assert_eq!(inv.cwd, PathBuf::from("/r/workspace"));
    let config_path = layout.host.join("mcp-config.json");
    assert_eq!(
        flag_value(&inv.args, "--mcp-config"),
        Some(config_path.to_string_lossy().as_ref())
    );
    assert!(inv.args.iter().any(|a| a == "--strict-mcp-config"));
    assert_eq!(
        flag_value(&inv.args, "--plugin-dir"),
        Some(plugin_dir.to_string_lossy().as_ref())
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&config_path).expect("config")).expect("json");
    assert_eq!(
        written,
        serde_json::json!({
            "mcpServers": {
                "local": {"type": "stdio", "command": "mcp-local", "args": ["--fast"], "env": {"K": "V"}},
                "remote": {"type": "http", "url": "https://mcp.example.test/mcp", "headers": {"Authorization": "Bearer t"}}
            }
        })
    );
}

#[test]
fn prepare_without_servers_writes_nothing_and_runs_in_the_scratch_workspace() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    let mut req = claude_request();
    req.workspace = PathBuf::new();
    let creds = crate::codex_cli::credentials();
    let ctx = PrepareContext {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://gw",
        hooks_file: None,
    };

    let inv = ClaudeCode.prepare(&ctx).expect("prepare");

    assert_eq!(inv.cwd, layout.workspace);
    assert!(!inv.args.iter().any(|a| a == "--mcp-config"));
    assert!(!inv.args.iter().any(|a| a == "--plugin-dir"));
    assert!(!layout.host.join("mcp-config.json").exists());
}

#[test]
fn prepare_reports_a_layout_error_when_the_host_dir_cannot_be_created() {
    let dir = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(dir.path());
    if let Some(parent) = layout.host.parent() {
        std::fs::create_dir_all(parent).expect("parent");
    }
    std::fs::write(&layout.host, b"blocker").expect("block host dir");
    let mut req = claude_request();
    req.mcp_servers = servers();
    let creds = crate::codex_cli::credentials();
    let ctx = PrepareContext {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://gw",
        hooks_file: None,
    };

    let err = ClaudeCode.prepare(&ctx).expect_err("host dir is a file");
    assert!(matches!(err, AgentRunsError::Layout { .. }), "got {err:?}");
}
