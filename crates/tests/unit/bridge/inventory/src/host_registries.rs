use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{
    AppInstallState, GeneratedProfile, HostApp, HostAppError, HostAppSnapshot, HostConfigSchema,
    ProbeEnv, ProfileGenInputs, ProfileInstalled, ProfileState,
};
use systemprompt_bridge::integration::{
    ResolvedHost, SYNC_ONLY_AGENTS, find_host_by_id, host_apps, resolve_host,
};
use systemprompt_bridge::{host_sync, register_host_app};
use systemprompt_models::bridge::host::HostKind;

// Why: this binary suppresses Hermes below (the white-label shape), so it is
// in neither the registry nor the sync-only table for every test here.
const SUPPRESSED: HostKind = HostKind::Hermes;

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
        .filter(|kind| *kind != SUPPRESSED)
        .filter(|kind| desktop_offered() || *kind != HostKind::ClaudeDesktop)
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
    for expected in HostKind::ALL {
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
        .filter(|s| s.host_id() == HostKind::ClaudeDesktop)
        .count();
    assert_eq!(
        cowork, 2,
        "the Cowork plugins and artifacts emitters share host_id \"claude-desktop\" and \
         must both survive dedup (dedup keys on concrete type, not host_id)"
    );
}

static TEST_SCHEMA: HostConfigSchema = HostConfigSchema {
    required_keys: &[],
    display_keys: &[],
};

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
    fn config_schema(&self) -> &'static HostConfigSchema {
        &TEST_SCHEMA
    }
    fn probe(&self, _env: &ProbeEnv) -> HostAppSnapshot {
        absent_snapshot(HostKind::CodexCli, "Shadowed Codex")
    }
    fn generate_profile(
        &self,
        _inputs: &ProfileGenInputs,
    ) -> Result<GeneratedProfile, HostAppError> {
        Ok(empty_profile())
    }
    fn install_profile(&self, _path: &str) -> Result<ProfileInstalled, HostAppError> {
        Ok(ProfileInstalled::ok())
    }
    fn install_action_label(&self) -> &'static str {
        "install"
    }
}

register_host_app!(ShadowCodexHost, priority = 100);

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
                ResolvedHost::Local(_) | ResolvedHost::SyncOnly(_) | ResolvedHost::Suppressed
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
fn a_kind_this_build_has_no_host_for_is_the_only_unknown() {
    if desktop_offered() {
        return;
    }
    assert!(matches!(
        resolve_host(HostKind::ClaudeDesktop),
        ResolvedHost::Unknown
    ));
}

#[test]
fn a_host_id_outside_the_closed_set_cannot_be_named() {
    assert!("no-such-agent".parse::<HostKind>().is_err());
    assert!("codex".parse::<HostKind>().is_err());
}

systemprompt_bridge::suppress_host_app!(SUPPRESSED);

// Why: this is the Astound shape — a white-label build calls
// `suppress_host_app!(HostKind::CodexCli)`, and the id is then in neither the
// registry nor the sync-only table. "Not offered on this installation" is the
// truthful answer; "unknown host" is not.
#[test]
fn suppressed_host_is_not_unknown() {
    assert!(find_host_by_id(SUPPRESSED).is_none());
    assert!(matches!(resolve_host(SUPPRESSED), ResolvedHost::Suppressed));
}
