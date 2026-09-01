//! `instance_enabled_hosts` gates the bridge host list on the operator's
//! `external_agents` catalog: an entry with `enabled: false` removes its host
//! instance-wide, an absent entry leaves the host enabled, and catalog ids
//! (snake_case) map onto host ids (kebab-case).

use systemprompt_api::routes::gateway::bridge::instance_enabled_hosts;
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_models::services::ServicesConfig;
use systemprompt_models::services::external_agent::{ExternalAgentConfig, ExternalAgentKind};

fn catalog_entry(id: &str, enabled: bool) -> (ExternalAgentId, ExternalAgentConfig) {
    let agent_id = ExternalAgentId::new(id);
    (
        agent_id.clone(),
        ExternalAgentConfig {
            id: agent_id,
            display_name: id.to_owned(),
            kind: ExternalAgentKind::CliTool,
            enabled,
            description: String::new(),
            platforms: Vec::new(),
            docs_url: None,
        },
    )
}

#[test]
fn empty_catalog_enables_every_known_host() {
    let services = ServicesConfig::default();
    assert_eq!(
        instance_enabled_hosts(&services),
        vec![
            "claude-code",
            "claude-desktop",
            "cowork",
            "codex-cli",
            "hermes",
            "opencode"
        ]
    );
}

#[test]
fn a_disabled_catalog_entry_removes_its_host() {
    let mut services = ServicesConfig::default();
    services
        .external_agents
        .extend([catalog_entry("codex_cli", false)]);
    assert_eq!(
        instance_enabled_hosts(&services),
        vec![
            "claude-code",
            "claude-desktop",
            "cowork",
            "hermes",
            "opencode"
        ]
    );
}

#[test]
fn an_enabled_catalog_entry_keeps_its_host() {
    let mut services = ServicesConfig::default();
    services.external_agents.extend([
        catalog_entry("codex_cli", true),
        catalog_entry("claude_desktop", true),
    ]);
    assert_eq!(
        instance_enabled_hosts(&services),
        vec![
            "claude-code",
            "claude-desktop",
            "cowork",
            "codex-cli",
            "hermes",
            "opencode"
        ]
    );
}

#[test]
fn snake_case_catalog_ids_map_onto_kebab_case_host_ids() {
    let mut services = ServicesConfig::default();
    services.external_agents.extend([
        catalog_entry("claude_desktop", false),
        catalog_entry("claude_code", false),
    ]);
    assert_eq!(
        instance_enabled_hosts(&services),
        vec!["cowork", "codex-cli", "hermes", "opencode"]
    );
}
