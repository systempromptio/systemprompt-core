//! `instance_enabled_hosts` gates the host list on the operator's
//! `external_agents` catalog. A host the bridge configures is served unless
//! its entry says `enabled: false`; a host that configures itself (`harness`)
//! is served only when its entry says `enabled: true`, and with no entry is
//! withheld from the instance hosts, the per-user manifest and consumer
//! authorization alike. Catalog ids (snake_case) map onto host ids
//! (kebab-case); `default_enabled: false` keeps an enabled host but makes it
//! opt-in. `host_extensions` is additive capability a host-owned host alone
//! may declare, and the manifest route carries it only for a host the user
//! has enabled.

use systemprompt_gateway_entry::http::bridge::manifest::hosts;
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_manifest::services::external_agent::{ExternalAgentConfig, ExternalAgentKind};
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};
use systemprompt_models::bridge::ids::HostExtensionId;
use systemprompt_models::bridge::manifest::{
    HOST_OWNED_HOST_MIN_BRIDGE, bridge_version_is_supported, manifest_min_bridge_version,
    min_bridge_version,
};
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_oauth::repository::EnabledHostPrefs;

fn keyed_entry(key: &str, id: &str, enabled: bool) -> (ExternalAgentId, ExternalAgentConfig) {
    (
        ExternalAgentId::try_new(key).expect("valid ExternalAgentId"),
        ExternalAgentConfig {
            id: ExternalAgentId::try_new(id).expect("valid ExternalAgentId"),
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

fn catalog_entry(id: &str, enabled: bool) -> (ExternalAgentId, ExternalAgentConfig) {
    keyed_entry(id, id, enabled)
}

fn bridge_configured() -> Vec<HostKind> {
    HostKind::ALL
        .into_iter()
        .filter(|host| host.config_owner() == ConfigOwner::Bridge)
        .collect()
}

fn declaring(entries: &[(&str, bool)]) -> ServicesConfig {
    let mut services = ServicesConfig::default();
    services.external_agents.extend(
        entries
            .iter()
            .map(|(id, enabled)| catalog_entry(id, *enabled)),
    );
    services
}

#[test]
fn an_empty_catalog_enables_every_bridge_configured_host_and_withholds_harness() {
    let services = ServicesConfig::default();
    assert_eq!(services.instance_enabled_hosts(), bridge_configured());
    assert!(
        !services
            .instance_enabled_hosts()
            .contains(&HostKind::Harness)
    );
    assert!(
        !services
            .instance_hosts()
            .enabled
            .contains(&HostKind::Harness)
    );
}

#[test]
fn a_host_that_configures_itself_is_served_only_when_declared_and_enabled() {
    let enabled = declaring(&[("harness", true)]);
    assert_eq!(enabled.instance_enabled_hosts(), HostKind::ALL.to_vec());
    enabled
        .validate()
        .expect("a declared harness entry validates");

    let disabled = declaring(&[("harness", false)]);
    assert_eq!(disabled.instance_enabled_hosts(), bridge_configured());

    for host in HostKind::ALL {
        let absent_means = ServicesConfig::default()
            .instance_enabled_hosts()
            .contains(&host);
        assert_eq!(
            absent_means,
            host.config_owner() == ConfigOwner::Bridge,
            "{host}: the default follows who owns the configuration, not the id"
        );
    }
}

#[test]
fn a_disabled_catalog_entry_removes_its_host() {
    let services = declaring(&[("codex_cli", false)]);
    assert_eq!(
        services.instance_enabled_hosts(),
        bridge_configured()
            .into_iter()
            .filter(|host| *host != HostKind::CodexCli)
            .collect::<Vec<_>>(),
        "disabling the CLI catalog entry leaves the Codex desktop app enabled"
    );
}

#[test]
fn an_enabled_catalog_entry_keeps_its_host() {
    let services = declaring(&[("codex_cli", true), ("claude_desktop", true)]);
    assert_eq!(services.instance_enabled_hosts(), bridge_configured());
}

#[test]
fn snake_case_catalog_ids_map_onto_kebab_case_host_ids() {
    let services = declaring(&[("claude_desktop", false), ("claude_code", false)]);
    assert_eq!(
        services.instance_enabled_hosts(),
        vec![
            HostKind::Codex,
            HostKind::CodexCli,
            HostKind::OpenCode,
            HostKind::OpenCodeCli,
            HostKind::Hermes,
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

#[test]
fn the_per_user_manifest_names_harness_only_on_an_instance_that_declares_it() {
    let no_rows = EnabledHostPrefs::default();

    let default_instance = ServicesConfig::default().instance_hosts();
    let hosts_by_default = hosts::user_enabled_hosts(&no_rows, &default_instance);
    assert_eq!(hosts_by_default, bridge_configured());
    assert_eq!(
        manifest_min_bridge_version(&[], hosts_by_default),
        min_bridge_version(),
        "a default instance asks nothing new of a deployed bridge"
    );

    let declared = declaring(&[("harness", true)]);
    let enabled_hosts = hosts::user_enabled_hosts(&no_rows, &declared.instance_hosts());
    assert!(enabled_hosts.contains(&HostKind::Harness));
    let protocols = hosts::host_model_protocols_for(Vec::new(), &declared.instance_hosts());
    let extensions = hosts::host_extensions_for(&declared, &enabled_hosts);
    let floor =
        manifest_min_bridge_version(&[], hosts::named(&enabled_hosts, &protocols, &extensions));
    assert_eq!(floor, HOST_OWNED_HOST_MIN_BRIDGE);
    assert!(
        !bridge_version_is_supported("0.87.0", &floor),
        "a bridge that cannot parse the host id gets the upgrade-required refusal"
    );
    assert!(bridge_version_is_supported("0.88.0", &floor));
}

#[test]
fn a_user_who_chose_other_hosts_gets_no_harness_and_no_raised_floor() {
    let declared = declaring(&[("harness", true)]);
    let instance = declared.instance_hosts();
    let chose_one = EnabledHostPrefs {
        hosts: vec![HostKind::ClaudeCode],
        unknown_enabled_row: false,
    };
    let enabled_hosts = hosts::user_enabled_hosts(&chose_one, &instance);
    assert_eq!(enabled_hosts, [HostKind::ClaudeCode]);
    let extensions = hosts::host_extensions_for(&declared, &enabled_hosts);
    assert_eq!(
        manifest_min_bridge_version(
            &[],
            hosts::named(&enabled_hosts, &Default::default(), &extensions)
        ),
        min_bridge_version()
    );
}

#[test]
fn a_stored_model_filter_for_a_withheld_host_stays_out_of_the_manifest() {
    let rows = vec![
        (HostKind::Codex, vec!["openai".to_owned()]),
        (HostKind::Harness, vec!["claude".to_owned()]),
    ];
    let default_instance = ServicesConfig::default().instance_hosts();
    let carried = hosts::host_model_protocols_for(rows.clone(), &default_instance);
    assert_eq!(
        carried.keys().copied().collect::<Vec<_>>(),
        [HostKind::Codex],
        "a stale row never names a host an older bridge cannot parse"
    );

    let declared = declaring(&[("harness", true)]).instance_hosts();
    let carried = hosts::host_model_protocols_for(rows, &declared);
    assert_eq!(
        carried.keys().copied().collect::<Vec<_>>(),
        [HostKind::Codex, HostKind::Harness]
    );
    assert_eq!(
        manifest_min_bridge_version(&[], hosts::named(&[], &carried, &Default::default())),
        HOST_OWNED_HOST_MIN_BRIDGE,
        "a host named only by a filter key still raises the floor"
    );
}

// Why: consumer authorization admits a plan or session binding for `host`
// only when an instance-enabled host accepts it; this is that predicate.
fn instance_serves(services: &ServicesConfig, host: EvaluatorClient) -> bool {
    services
        .instance_enabled_hosts()
        .iter()
        .any(|enabled| host.accepts(*enabled))
}

#[test]
fn consumer_authorization_for_harness_follows_the_instance_declaration() {
    assert!(
        !instance_serves(&ServicesConfig::default(), EvaluatorClient::Harness),
        "host=harness is refused on an instance that declares nothing"
    );
    assert!(!instance_serves(
        &declaring(&[("harness", false)]),
        EvaluatorClient::Harness
    ));
    assert!(
        instance_serves(&declaring(&[("harness", true)]), EvaluatorClient::Harness),
        "host=harness is admitted once the instance enables it"
    );
    assert!(instance_serves(
        &ServicesConfig::default(),
        EvaluatorClient::ClaudeCode
    ));
}

fn extension(id: &str) -> HostExtensionId {
    HostExtensionId::try_new(id).expect("kebab-case extension id")
}

fn with_extensions(id: &str, extensions: &[&str]) -> (ExternalAgentId, ExternalAgentConfig) {
    let (key, mut agent) = catalog_entry(id, true);
    agent.extensions = extensions.iter().map(|ext| extension(ext)).collect();
    (key, agent)
}

#[test]
fn host_extensions_follow_the_catalog_entry_of_each_enabled_host() {
    let mut services = ServicesConfig::default();
    assert!(services.instance_host_extensions().is_empty());
    services.external_agents.extend([
        with_extensions("harness", &["drive", "brain", "canvas"]),
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
        [extension("drive"), extension("brain"), extension("canvas")]
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
fn the_manifest_carries_extensions_only_for_a_host_the_user_has_enabled() {
    let mut services = ServicesConfig::default();
    services
        .external_agents
        .extend([with_extensions("harness", &["drive", "brain"])]);

    let everything = hosts::host_extensions_for(&services, &HostKind::ALL);
    assert_eq!(
        everything.get(&HostKind::Harness),
        Some(&vec![extension("drive"), extension("brain")])
    );

    let without_harness = hosts::host_extensions_for(&services, &bridge_configured());
    assert!(
        without_harness.is_empty(),
        "absent means off: a user without the host is told of no extension"
    );
    assert!(hosts::host_extensions_for(&services, &[]).is_empty());
}

#[test]
fn only_a_host_that_configures_itself_may_declare_extensions() {
    for id in ["codex_cli", "claude_desktop", "hermes", "cursor"] {
        let mut services = ServicesConfig::default();
        services
            .external_agents
            .extend([with_extensions(id, &["drive"])]);
        let err = services
            .validate()
            .expect_err("extensions on a host the bridge configures mean nothing");
        assert!(
            err.to_string()
                .contains("names no host that configures itself"),
            "{id}: {err}"
        );
        assert!(
            services.instance_host_extensions().is_empty(),
            "{id}: an unvalidated tree still projects no extension for it"
        );
    }
}

#[test]
fn a_repeated_extension_fails_validation() {
    let mut services = ServicesConfig::default();
    services
        .external_agents
        .extend([with_extensions("harness", &["drive", "drive"])]);
    let err = services
        .validate()
        .expect_err("a repeated extension must not validate");
    assert!(err.to_string().contains("listed twice"), "{err}");
}

#[test]
fn a_malformed_extension_id_is_refused_when_the_catalog_is_loaded() {
    for id in ["Drive", "", "9lives", "a-", "a--b", "-a"] {
        let err = serde_json::from_value::<ExternalAgentConfig>(serde_json::json!({
            "id": "harness",
            "display_name": "Harness",
            "kind": "desktop_app",
            "enabled": true,
            "extensions": [id]
        }))
        .expect_err("a malformed extension id must not load");
        assert!(err.to_string().contains("kebab-case"), "{id:?}: {err}");
    }
    let loaded: ExternalAgentConfig = serde_json::from_value(serde_json::json!({
        "id": "harness",
        "display_name": "Harness",
        "kind": "desktop_app",
        "enabled": true,
        "extensions": ["drive", "drive-sync"]
    }))
    .expect("kebab-case ids load");
    assert_eq!(
        loaded.extensions,
        [extension("drive"), extension("drive-sync")]
    );
}

#[test]
fn two_catalog_keys_naming_one_host_fail_validation() {
    let mut services = ServicesConfig::default();
    services.external_agents.extend([
        catalog_entry("codex_cli", true),
        catalog_entry("codex-cli", false),
    ]);
    let err = services
        .validate()
        .expect_err("which entry supplies `enabled` would follow map order");
    let message = err.to_string();
    assert!(
        message.contains("both name the host 'codex-cli'"),
        "{message}"
    );
    assert!(message.contains("codex_cli"), "{message}");
}

#[test]
fn a_catalog_key_that_differs_from_its_entry_id_fails_validation() {
    let mut services = ServicesConfig::default();
    services
        .external_agents
        .extend([keyed_entry("codex_cli", "claude_code", true)]);
    let err = services
        .validate()
        .expect_err("the key and the id of an entry must agree");
    assert!(
        err.to_string()
            .contains("id 'claude_code' differs from its key 'codex_cli'"),
        "{err}"
    );

    let mut agreeing = ServicesConfig::default();
    agreeing.external_agents.extend([
        catalog_entry("codex_cli", true),
        catalog_entry("claude_desktop", true),
        catalog_entry("harness", true),
    ]);
    agreeing.validate().expect("agreeing keys and ids validate");
}
