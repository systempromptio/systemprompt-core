use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{
    AppInstallState, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema, ProbeEnv,
    ProfileGenInputs, ProfileState,
};
use systemprompt_bridge::integration::{find_host_by_id, host_apps};
use systemprompt_bridge::{host_sync, register_host_app};

#[test]
fn host_apps_contains_builtins() {
    let ids: Vec<&str> = host_apps().iter().map(|h| h.id()).collect();
    for expected in ["codex-cli", "hermes", "opencode"] {
        assert!(
            ids.contains(&expected),
            "{expected} built-in host missing; registry = {ids:?}"
        );
    }
}

// Why: the gateway only offers the bridge hosts it knows; a host registered
// here but absent there is never enabled, and one known there with no
// implementation here silently vanishes from the GUI.
#[test]
fn known_hosts_cover_every_local_and_sync_only_agent() {
    use systemprompt_bridge::integration::SYNC_ONLY_AGENTS;
    use systemprompt_models::bridge::profile::KNOWN_HOSTS;

    let mut bridge: Vec<&str> = host_apps()
        .iter()
        .map(|h| h.id())
        .filter(|id| !id.starts_with("dummy-"))
        .chain(SYNC_ONLY_AGENTS.iter().map(|a| a.id))
        .collect();
    bridge.sort_unstable();
    bridge.dedup();
    let mut known: Vec<&str> = KNOWN_HOSTS.to_vec();
    if !cfg!(any(target_os = "macos", target_os = "windows")) {
        known.retain(|id| *id != "claude-desktop");
    }
    known.sort_unstable();
    assert_eq!(
        bridge, known,
        "bridge registries and the gateway KNOWN_HOSTS list have drifted"
    );
}

#[test]
fn host_apps_are_sorted_by_id() {
    let ids: Vec<&str> = host_apps().iter().map(|h| h.id()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "host registry must be sorted by id");
}

#[test]
fn host_sync_registry_contains_builtins() {
    let ids: Vec<&str> = host_sync::registry().iter().map(|s| s.host_id()).collect();
    for expected in [
        "codex-cli",
        "claude-code",
        "claude-desktop",
        "hermes",
        "opencode",
    ] {
        assert!(
            ids.contains(&expected),
            "{expected} host sync missing; registry = {ids:?}"
        );
    }
}

#[test]
fn host_sync_registry_keeps_both_claude_desktop_facets() {
    let cowork = host_sync::registry()
        .iter()
        .filter(|s| s.host_id() == "claude-desktop")
        .count();
    assert_eq!(
        cowork, 2,
        "the Cowork plugins and artifacts emitters share host_id \"claude-desktop\" and \
         must both survive dedup (dedup keys on concrete type, not host_id)"
    );
}

struct DummyHost;

static DUMMY_SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: &[],
    display_keys: &[],
};

impl HostApp for DummyHost {
    fn id(&self) -> &'static str {
        "dummy-test-host"
    }
    fn display_name(&self) -> &'static str {
        "Dummy Test Host"
    }
    fn config_schema(&self) -> &'static HostConfigSchema {
        &DUMMY_SCHEMA
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: "dummy-test-host",
            display_name: "Dummy Test Host",
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            host_running: false,
            host_processes: Vec::new(),
            app_installed: AppInstallState::NotInstalled,
            probed_at_unix: 0,
        }
    }
    fn generate_profile(&self, _inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        Ok(GeneratedProfile {
            path: String::new(),
            bytes: 0,
            payload_uuid: String::new(),
            profile_uuid: String::new(),
        })
    }
    fn install_profile(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

register_host_app!(DummyHost);

#[test]
fn externally_registered_host_is_discoverable() {
    let host = find_host_by_id("dummy-test-host");
    assert!(
        host.is_some(),
        "host registered via register_host_app! not found in registry"
    );
    assert_eq!(host.unwrap().display_name(), "Dummy Test Host");
}

struct ShadowCodexHost;

impl HostApp for ShadowCodexHost {
    fn id(&self) -> &'static str {
        "codex-cli"
    }
    fn display_name(&self) -> &'static str {
        "Shadowed Codex"
    }
    fn config_schema(&self) -> &'static HostConfigSchema {
        &DUMMY_SCHEMA
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: "codex-cli",
            display_name: "Shadowed Codex",
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            host_running: false,
            host_processes: Vec::new(),
            app_installed: AppInstallState::NotInstalled,
            probed_at_unix: 0,
        }
    }
    fn generate_profile(&self, _inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        Ok(GeneratedProfile {
            path: String::new(),
            bytes: 0,
            payload_uuid: String::new(),
            profile_uuid: String::new(),
        })
    }
    fn install_profile(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

register_host_app!(ShadowCodexHost, priority = 100);

#[test]
fn higher_priority_registration_shadows_builtin() {
    let host = find_host_by_id("codex-cli").expect("codex-cli present");
    assert_eq!(
        host.display_name(),
        "Shadowed Codex",
        "priority-100 registration should shadow the built-in codex-cli host"
    );
    let count = host_apps().iter().filter(|h| h.id() == "codex-cli").count();
    assert_eq!(count, 1, "shadowed id must appear exactly once (deduped)");
}

// Why: v0.43.0 toasted `unknown host: claude-code` from seven handlers at once,
// each of which had re-derived "is this id real" for itself. `resolve_host` is
// the one place that decision is made now, so this is the one place it is
// asserted: no id the gateway may send can come back Unknown.
#[test]
fn no_known_host_resolves_as_unknown() {
    use systemprompt_bridge::integration::{ResolvedHost, resolve_host};
    use systemprompt_models::bridge::profile::KNOWN_HOSTS;

    for id in KNOWN_HOSTS {
        if !cfg!(any(target_os = "macos", target_os = "windows")) && *id == "claude-desktop" {
            continue;
        }
        assert!(
            matches!(
                resolve_host(id),
                ResolvedHost::Local(_) | ResolvedHost::SyncOnly(_) | ResolvedHost::Suppressed
            ),
            "{id} resolves as Unknown — a per-host command for it would answer \
             \"unknown host: {id}\""
        );
    }
}

#[test]
fn sync_only_agent_resolves_without_a_host_app() {
    use systemprompt_bridge::integration::{ResolvedHost, resolve_host};

    assert!(
        find_host_by_id("claude-code").is_none(),
        "sync-only by design"
    );
    let ResolvedHost::SyncOnly(agent) = resolve_host("claude-code") else {
        panic!("claude-code must resolve as a sync-only agent, not an unknown id");
    };
    assert_eq!(agent.display_name, "Claude Code");
}

#[test]
fn an_id_belonging_to_nothing_is_the_only_unknown() {
    use systemprompt_bridge::integration::{ResolvedHost, resolve_host};

    assert!(matches!(
        resolve_host("no-such-agent"),
        ResolvedHost::Unknown
    ));
}

struct SuppressedHost;

impl HostApp for SuppressedHost {
    fn id(&self) -> &'static str {
        "dummy-suppressed-host"
    }
    fn display_name(&self) -> &'static str {
        "Suppressed Host"
    }
    fn config_schema(&self) -> &'static HostConfigSchema {
        &DUMMY_SCHEMA
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        HostAppSnapshot {
            host_id: "dummy-suppressed-host",
            display_name: "Suppressed Host",
            profile_state: ProfileState::Absent,
            profile_source: None,
            profile_keys: BTreeMap::new(),
            host_running: false,
            host_processes: Vec::new(),
            app_installed: AppInstallState::NotInstalled,
            probed_at_unix: 0,
        }
    }
    fn generate_profile(&self, _inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
        Ok(GeneratedProfile {
            path: String::new(),
            bytes: 0,
            payload_uuid: String::new(),
            profile_uuid: String::new(),
        })
    }
    fn install_profile(&self, _path: &str) -> std::io::Result<()> {
        Ok(())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

register_host_app!(SuppressedHost);
systemprompt_bridge::suppress_host_app!("dummy-suppressed-host");

// Why: this is the Astound shape — a white-label build calls
// `suppress_host_app!("codex-cli")`, and the id is then in neither the registry
// nor the sync-only table. "Not offered on this installation" is the truthful
// answer; "unknown host" is not.
#[test]
fn suppressed_host_is_not_unknown() {
    use systemprompt_bridge::integration::{ResolvedHost, resolve_host};

    assert!(find_host_by_id("dummy-suppressed-host").is_none());
    assert!(matches!(
        resolve_host("dummy-suppressed-host"),
        ResolvedHost::Suppressed
    ));
}
