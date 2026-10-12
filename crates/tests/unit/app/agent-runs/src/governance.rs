//! Golden tests for the governance hooks file: Claude Code gets a plugin
//! dir with `plugin.json` and `hooks/hooks.json`; Codex CLI gets nothing.

use systemprompt_agent_runs::governance::hooks_schema::{HooksFile, PluginManifest, TRACK_EVENTS};
use systemprompt_agent_runs::governance::{HookBases, hooks_file};
use systemprompt_agent_runs::{HostKind, RunLayout};
use systemprompt_identifiers::PluginId;

fn bases() -> HookBases {
    HookBases {
        govern_url: "http://gw.test/api/public/hooks/govern".to_owned(),
        track_url: "http://gw.test/api/public/hooks/track".to_owned(),
    }
}

#[tokio::test]
async fn claude_code_gets_a_governance_plugin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(tmp.path().join("run"));
    let dir = hooks_file(
        HostKind::ClaudeCode,
        &PluginId::new("farm-gov"),
        "tok-123",
        &bases(),
        &layout,
    )
    .await
    .expect("write hooks")
    .expect("claude code gets a plugin dir");
    assert!(dir.starts_with(&layout.host));

    let manifest: PluginManifest = serde_json::from_slice(
        &std::fs::read(dir.join(".claude-plugin/plugin.json")).expect("manifest"),
    )
    .expect("manifest json");
    assert_eq!(manifest.name, "headless-governance");

    let raw = std::fs::read_to_string(dir.join("hooks/hooks.json")).expect("hooks");
    let hooks: HooksFile = serde_json::from_str(&raw).expect("hooks json");
    assert_eq!(hooks.hooks.len(), TRACK_EVENTS.len() + 1);

    let pre = &hooks.hooks["PreToolUse"][0];
    assert_eq!(pre.matcher, "*");
    let govern = &pre.hooks[0];
    assert_eq!(govern.kind, "http");
    assert_eq!(
        govern.url,
        "http://gw.test/api/public/hooks/govern?plugin_id=farm-gov"
    );
    assert_eq!(govern.headers["Authorization"], "Bearer tok-123");
    assert_eq!(govern.timeout, 10);
    assert_eq!(govern.r#async, None);
    assert_eq!(govern.event, None);

    for event in TRACK_EVENTS {
        let track = &hooks.hooks[*event][0].hooks[0];
        assert_eq!(
            track.url,
            "http://gw.test/api/public/hooks/track?plugin_id=farm-gov"
        );
        assert_eq!(track.r#async, Some(true));
        assert_eq!(track.event.as_deref(), Some(*event));
        assert_eq!(track.headers["Authorization"], "Bearer tok-123");
    }

    let value: serde_json::Value = serde_json::from_str(&raw).expect("value");
    assert_eq!(value["hooks"]["Stop"][0]["hooks"][0]["type"], "http");
    assert_eq!(
        value["hooks"]["Stop"][0]["hooks"][0]["allowedEnvVars"],
        serde_json::json!([])
    );
}

#[tokio::test]
async fn codex_cli_gets_no_hooks_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let layout = RunLayout::new(tmp.path().join("run"));
    let out = hooks_file(
        HostKind::CodexCli,
        &PluginId::new("farm-gov"),
        "tok",
        &bases(),
        &layout,
    )
    .await
    .expect("no error");
    assert!(out.is_none());
    assert!(!layout.host.exists());
}
