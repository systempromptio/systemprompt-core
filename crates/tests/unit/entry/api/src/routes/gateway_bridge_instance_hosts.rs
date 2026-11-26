//! `instance_enabled_hosts` gates the bridge host list on the operator's
//! `external_agents` catalog: an entry with `enabled: false` removes its host
//! instance-wide, an absent entry leaves the host enabled, and catalog ids
//! (snake_case) map onto host ids (kebab-case). `default_enabled: false`
//! keeps an enabled host but makes it opt-in.

use systemprompt_identifiers::ExternalAgentId;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_manifest::services::external_agent::{ExternalAgentConfig, ExternalAgentKind};
use systemprompt_models::bridge::host::HostKind;

fn catalog_entry(id: &str, enabled: bool) -> (ExternalAgentId, ExternalAgentConfig) {
    let agent_id = ExternalAgentId::try_new(id).expect("valid ExternalAgentId");
    (
        agent_id.clone(),
        ExternalAgentConfig {
            id: agent_id,
            display_name: id.to_owned(),
            kind: ExternalAgentKind::CliTool,
            enabled,
            default_enabled: true,
            description: String::new(),
            platforms: Vec::new(),
            docs_url: None,
            desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy::default(),
            extensions: Vec::new(),
        },
    )
}

#[test]
fn empty_catalog_enables_every_known_host() {
    let services = ServicesConfig::default();
    assert_eq!(services.instance_enabled_hosts(), HostKind::ALL.to_vec());
}

#[test]
fn a_disabled_catalog_entry_removes_its_host() {
    let mut services = ServicesConfig::default();
    services
        .external_agents
        .extend([catalog_entry("codex_cli", false)]);
    assert_eq!(
        services.instance_enabled_hosts(),
        HostKind::ALL
            .into_iter()
            .filter(|host| *host != HostKind::CodexCli)
            .collect::<Vec<_>>(),
        "disabling the CLI catalog entry leaves the Codex desktop app enabled"
    );
}

#[test]
fn an_enabled_catalog_entry_keeps_its_host() {
    let mut services = ServicesConfig::default();
    services.external_agents.extend([
        catalog_entry("codex_cli", true),
        catalog_entry("claude_desktop", true),
    ]);
    assert_eq!(services.instance_enabled_hosts(), HostKind::ALL.to_vec());
}

#[test]
fn snake_case_catalog_ids_map_onto_kebab_case_host_ids() {
    let mut services = ServicesConfig::default();
    services.external_agents.extend([
        catalog_entry("claude_desktop", false),
        catalog_entry("claude_code", false),
    ]);
    assert_eq!(
        services.instance_enabled_hosts(),
        vec![
            HostKind::Codex,
            HostKind::CodexCli,
            HostKind::OpenCode,
            HostKind::OpenCodeCli,
            HostKind::Hermes,
            HostKind::Harness,
        ]
    );
}

#[test]
fn a_default_off_catalog_entry_is_enabled_but_opt_in() {
    let mut services = ServicesConfig::default();
    let (id, mut agent) = catalog_entry("claude_code", true);
    agent.default_enabled = false;
    services
        .external_agents
        .extend([(id, agent), catalog_entry("hermes", false)]);

    let instance = services.instance_hosts();
    assert!(instance.enabled.contains(&HostKind::ClaudeCode));
    assert!(instance.is_opt_in(HostKind::ClaudeCode));
    assert!(!instance.enabled.contains(&HostKind::Hermes));
    assert!(
        !instance.is_opt_in(HostKind::Hermes),
        "a disabled host is not offered at all"
    );
    assert!(
        instance
            .by_default()
            .all(|host| host != HostKind::ClaudeCode)
    );
}

#[test]
fn a_catalog_entry_without_default_enabled_reaches_every_user() {
    let file: ExternalAgentConfig = serde_json::from_value(serde_json::json!({
        "id": "claude_code",
        "display_name": "Claude Code",
        "kind": "cli_tool",
        "enabled": true
    }))
    .expect("parses");
    assert!(file.default_enabled);
}

fn with_extensions(id: &str, extensions: &[&str]) -> (ExternalAgentId, ExternalAgentConfig) {
    let (key, mut agent) = catalog_entry(id, true);
    agent.extensions = extensions.iter().map(|ext| (*ext).to_owned()).collect();
    (key, agent)
}

#[test]
fn host_extensions_follow_the_catalog_entry_of_each_enabled_host() {
    let mut services = ServicesConfig::default();
    assert!(services.instance_host_extensions().is_empty());
    services.external_agents.extend([
        with_extensions("harness", &["governance", "drive", "brain"]),
        with_extensions("codex_cli", &[]),
    ]);
    let extensions = services.instance_host_extensions();
    assert_eq!(
        extensions.keys().copied().collect::<Vec<_>>(),
        [HostKind::Harness],
        "a host with no extensions is absent, not an empty list"
    );
    assert_eq!(
        extensions[&HostKind::Harness],
        ["governance", "drive", "brain"]
    );
    services
        .validate()
        .expect("well-formed extension ids validate");
}

#[test]
fn a_disabled_host_carries_no_extensions() {
    let mut services = ServicesConfig::default();
    let (key, mut agent) = with_extensions("harness", &["drive"]);
    agent.enabled = false;
    services.external_agents.insert(key, agent);
    assert!(services.instance_host_extensions().is_empty());
}

#[test]
fn malformed_repeated_or_hostless_extensions_fail_validation() {
    for (id, extensions, needle) in [
        ("harness", &["Drive"][..], "lowercase"),
        ("harness", &["drive", "drive"][..], "listed twice"),
        ("harness", &[""][..], "lowercase"),
        ("harness", &["9lives"][..], "lowercase"),
        ("cursor", &["drive"][..], "names no known host"),
    ] {
        let mut services = ServicesConfig::default();
        services
            .external_agents
            .extend([with_extensions(id, extensions)]);
        let err = services
            .validate()
            .expect_err("an invalid extension list must not validate");
        assert!(
            err.to_string().contains(needle),
            "{id} {extensions:?}: {err}"
        );
    }
}
