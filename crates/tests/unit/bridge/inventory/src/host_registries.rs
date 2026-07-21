use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{
    AppInstallState, GeneratedProfile, HostApp, HostAppSnapshot, HostConfigSchema,
    ProfileGenInputs, ProfileState,
};
use systemprompt_bridge::integration::{find_host_by_id, host_apps};
use systemprompt_bridge::register_host_app;
use systemprompt_bridge::sync::host_sync;

#[test]
fn host_apps_contains_builtins() {
    let ids: Vec<&str> = host_apps().iter().map(|h| h.id()).collect();
    assert!(
        ids.contains(&"codex-cli"),
        "codex-cli built-in host missing; registry = {ids:?}"
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
    for expected in ["codex-cli", "claude-code", "cowork"] {
        assert!(
            ids.contains(&expected),
            "{expected} host sync missing; registry = {ids:?}"
        );
    }
}

#[test]
fn host_sync_registry_keeps_both_cowork_facets() {
    let cowork = host_sync::registry()
        .iter()
        .filter(|s| s.host_id() == "cowork")
        .count();
    assert_eq!(
        cowork, 2,
        "the Cowork plugins and artifacts emitters share host_id \"cowork\" and \
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
    fn probe(&self) -> HostAppSnapshot {
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
    fn probe(&self) -> HostAppSnapshot {
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
