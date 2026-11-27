use std::collections::BTreeMap;

use systemprompt_bridge::drift::DesiredProfile;
use systemprompt_bridge::integration::host_app::{
    AppInstallState, GeneratedProfile, HostApp, HostAppError, HostAppSnapshot, ProbeEnv,
    ProfileGenInputs, ProfileInstalled, ProfileState,
};
use systemprompt_bridge::integration::{
    ResolvedHost, SYNC_ONLY_AGENTS, find_host_by_id, host_apps, resolve_host,
};
use systemprompt_bridge::probe_cache::StartMenuCache;
use systemprompt_bridge::{host_sync, register_host_app};
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};

fn desktop_offered() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

#[test]
fn host_apps_contains_builtins() {
    let ids: Vec<HostKind> = host_apps().iter().map(|h| h.id()).collect();
    for expected in [HostKind::CodexCli, HostKind::OpenCode] {
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
    let mut bridge: Vec<HostKind> = host_apps()
        .iter()
        .map(|h| h.id())
        .chain(SYNC_ONLY_AGENTS.iter().map(|a| a.id))
        .collect();
    bridge.sort_unstable();
    bridge.dedup();
    let mut known: Vec<HostKind> = HostKind::ALL
        .into_iter()
        .filter(|kind| desktop_offered() || *kind != HostKind::ClaudeDesktop)
        .filter(|kind| kind.config_owner() == ConfigOwner::Bridge)
        .collect();
    known.sort_unstable();
    assert_eq!(
        bridge, known,
        "bridge registries and the gateway HostKind set have drifted"
    );
}

#[test]
fn host_apps_are_sorted_by_id() {
    let ids: Vec<HostKind> = host_apps().iter().map(|h| h.id()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "host registry must be sorted by id");
}

#[test]
fn host_sync_registry_contains_builtins() {
    let ids: Vec<HostKind> = host_sync::registry().iter().map(|s| s.host_id()).collect();
    for expected in HostKind::ALL
        .into_iter()
        .filter(|kind| kind.config_owner() == ConfigOwner::Bridge)
    {
        assert!(
            host_sync::registry()
                .iter()
                .any(|emitter| emitter.serves(expected)),
            "{expected} host sync missing; registry = {ids:?}"
        );
    }
    for (product_a, product_b) in [
        (HostKind::Codex, HostKind::CodexCli),
        (HostKind::OpenCode, HostKind::OpenCodeCli),
    ] {
        let shared: Vec<&&dyn host_sync::HostSync> = host_sync::registry()
            .iter()
            .filter(|emitter| emitter.serves(product_a) && emitter.serves(product_b))
            .collect();
        assert_eq!(
            shared.len(),
            1,
            "{product_a} and {product_b} share exactly one config emitter"
        );
    }
}

#[test]
fn host_sync_registry_keeps_every_claude_desktop_facet() {
    let emitters: std::collections::BTreeSet<&str> = host_sync::registry()
        .iter()
        .filter(|s| s.host_id() == HostKind::ClaudeDesktop)
        .map(|s| s.emitter_id())
        .collect();
    let mut expected = std::collections::BTreeSet::from(["cowork-artifacts", "cowork-plugins"]);
    if desktop_offered() {
        expected.insert(HostKind::ClaudeDesktop.as_str());
    }
    assert_eq!(
        emitters, expected,
        "the Cowork plugins and artifacts emitters (and the MDM policy sync where Claude \
         Desktop ships) share host_id \"claude-desktop\" and must all survive dedup (dedup \
         keys on concrete type, not host_id)"
    );
}

fn absent_snapshot(host_id: HostKind, display_name: &'static str) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id,
        display_name,
        profile_state: ProfileState::Absent,
        profile_source: None,
        profile_keys: BTreeMap::new(),
        probe_error: None,
        host_running: Some(false),
        host_processes: Vec::new(),
        app_installed: AppInstallState::NotInstalled,
        probed_at_unix: 0,
        update_needs_approval: false,
    }
}

fn empty_profile() -> GeneratedProfile {
    GeneratedProfile {
        path: String::new(),
        bytes: 0,
        payload_uuid: String::new(),
        profile_uuid: String::new(),
    }
}

struct ShadowCodexHost;

impl HostApp for ShadowCodexHost {
    fn id(&self) -> HostKind {
        HostKind::CodexCli
    }
    fn display_name(&self) -> &'static str {
        "Shadowed Codex"
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        absent_snapshot(HostKind::CodexCli, "Shadowed Codex")
    }
    fn desired(&self, _inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        Ok(DesiredProfile::new())
    }
    fn app_installed(&self, _start_menu: &StartMenuCache) -> AppInstallState {
        AppInstallState::NotInstalled
    }
    fn generate_profile(
        &self,
        _inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        Ok(empty_profile())
    }
    fn install_profile(
        &self,
        _path: &str,
        _gate: &systemprompt_bridge::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        Ok(ProfileInstalled::ok())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

register_host_app!(ShadowCodexHost, priority = 100);

struct SelfConfiguringHost {
    id: HostKind,
    profile_identity: HostKind,
    display_name: &'static str,
}

impl HostApp for SelfConfiguringHost {
    fn id(&self) -> HostKind {
        self.id
    }
    fn profile_identity(&self) -> HostKind {
        self.profile_identity
    }
    fn display_name(&self) -> &'static str {
        self.display_name
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        absent_snapshot(self.id, self.display_name)
    }
    fn desired(&self, _inputs: &ProfileGenInputs) -> Result<DesiredProfile, HostAppError> {
        Ok(DesiredProfile::new())
    }
    fn app_installed(&self, _start_menu: &StartMenuCache) -> AppInstallState {
        AppInstallState::NotInstalled
    }
    fn generate_profile(
        &self,
        _inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        Ok(empty_profile())
    }
    fn install_profile(
        &self,
        _path: &str,
        _gate: &systemprompt_bridge::elevation_gate::ElevationGate,
    ) -> Result<ProfileInstalled, HostAppError> {
        Ok(ProfileInstalled::ok())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

static REGISTERED_HARNESS: SelfConfiguringHost = SelfConfiguringHost {
    id: HostKind::Harness,
    profile_identity: HostKind::Harness,
    display_name: "Registered Harness",
};
static HERMES_WITH_HARNESS_IDENTITY: SelfConfiguringHost = SelfConfiguringHost {
    id: HostKind::Hermes,
    profile_identity: HostKind::Harness,
    display_name: "Hermes With Harness Identity",
};

register_host_app!(REGISTERED_HARNESS, priority = 100);
register_host_app!(HERMES_WITH_HARNESS_IDENTITY, priority = 100);

#[test]
fn a_registration_for_a_host_that_owns_its_configuration_is_refused() {
    assert!(
        find_host_by_id(HostKind::Harness).is_none(),
        "a crate registering a host app for harness must not make it enrollable"
    );
    assert!(matches!(
        resolve_host(HostKind::Harness),
        ResolvedHost::Unmanaged(HostKind::Harness)
    ));
    let hermes = find_host_by_id(HostKind::Hermes).expect("the built-in hermes host remains");
    assert_ne!(
        hermes.display_name(),
        "Hermes With Harness Identity",
        "a registration whose profile would carry a harness credential is refused, \
         whatever id it registers under"
    );
    assert!(
        host_apps()
            .iter()
            .all(|host| host.profile_identity().config_owner() == ConfigOwner::Bridge),
        "no listed host app renders a profile for a host that owns its configuration"
    );
}

#[test]
fn every_host_that_owns_its_configuration_is_unmanaged_and_unserved() {
    for id in HostKind::ALL
        .into_iter()
        .filter(|kind| kind.config_owner() == ConfigOwner::Host)
    {
        assert!(
            matches!(resolve_host(id), ResolvedHost::Unmanaged(resolved) if resolved == id),
            "{id}"
        );
        assert!(find_host_by_id(id).is_none(), "{id} has no host app");
        assert!(
            SYNC_ONLY_AGENTS.iter().all(|agent| agent.id != id),
            "{id} is not a sync-only agent"
        );
        assert!(
            host_sync::registry()
                .iter()
                .all(|emitter| emitter.host_id() != id && !emitter.serves(id)),
            "no sync emitter writes for {id}"
        );
    }
}

#[test]
fn an_externally_registered_host_is_discoverable_and_shadows_the_builtin() {
    let host = find_host_by_id(HostKind::CodexCli).expect("codex-cli present");
    assert_eq!(
        host.display_name(),
        "Shadowed Codex",
        "priority-100 registration should shadow the built-in codex-cli host"
    );
    let count = host_apps()
        .iter()
        .filter(|h| h.id() == HostKind::CodexCli)
        .count();
    assert_eq!(count, 1, "shadowed id must appear exactly once (deduped)");
}

// Why: v0.43.0 toasted `unknown host: claude-code` from seven handlers at once,
// each of which had re-derived "is this id real" for itself. `resolve_host` is
// the one place that decision is made now, so this is the one place it is
// asserted: no id the gateway may send can come back Unknown.
#[test]
fn no_known_host_resolves_as_unknown() {
    for id in HostKind::ALL {
        if !desktop_offered() && id == HostKind::ClaudeDesktop {
            continue;
        }
        assert!(
            matches!(
                resolve_host(id),
                ResolvedHost::Local(_) | ResolvedHost::SyncOnly(_) | ResolvedHost::Unmanaged(_)
            ),
            "{id} resolves as Unknown — a per-host command for it would answer \
             \"unknown host: {id}\""
        );
    }
}

#[test]
fn sync_only_agent_resolves_without_a_host_app() {
    assert!(
        find_host_by_id(HostKind::ClaudeCode).is_none(),
        "sync-only by design"
    );
    let ResolvedHost::SyncOnly(agent) = resolve_host(HostKind::ClaudeCode) else {
        panic!("claude-code must resolve as a sync-only agent, not an unknown id");
    };
    assert_eq!(agent.display_name, "Claude Code");
}

#[test]
fn harness_is_known_but_unmanaged() {
    assert!(find_host_by_id(HostKind::Harness).is_none());
    assert!(
        matches!(
            resolve_host(HostKind::Harness),
            ResolvedHost::Unmanaged(HostKind::Harness)
        ),
        "harness reads the manifest itself; the bridge configures nothing for it"
    );
    assert!(
        host_apps()
            .iter()
            .all(|host| host.id() != HostKind::Harness),
        "harness is never listed as a host app"
    );
}

#[test]
fn a_kind_this_build_has_no_host_for_is_the_only_unknown() {
    let resolved = resolve_host(HostKind::ClaudeDesktop);
    assert_eq!(
        matches!(resolved, ResolvedHost::Unknown),
        !desktop_offered(),
        "claude-desktop is unknown exactly when this build offers no desktop host"
    );
}

#[test]
fn a_host_id_outside_the_closed_set_cannot_be_named() {
    assert!("no-such-agent".parse::<HostKind>().is_err());
    assert!("open-code".parse::<HostKind>().is_err());
}
