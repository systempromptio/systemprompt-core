//! Guards on the agent-health verdict — the single derivation that replaced
//! three divergent JavaScript folds.
//!
//! Two classes of guard here. First, the precedence ladder itself, including
//! the two corrections that motivated the move: a never-probed host is
//! `Checking`, not `Absent`, and an inconclusive `AppInstallState::Unknown` is
//! not `NotInstalled`. Second, that the fleet fold cannot contradict the rows
//! it was folded from — the bug that let the summary card say "all configured"
//! while the Agents tab said "Not working".
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_bridge::integration::agent_health::{
    AgentAction, AgentFleetSummary, AgentState, AgentSurface, AgentVerdict, FleetHeadline,
    FleetState, HostCapabilities, HostHealthInputs, HostModelViewRef, SYNC_ONLY_AGENTS, verdict,
};
use systemprompt_bridge::integration::host_app::{
    AppInstallState, HostAppSnapshot, ProfileState, StaleReason,
};
use systemprompt_bridge::proxy_probe::{ProxyHealth, ProxyProbeState};

const fn snapshot(profile_state: ProfileState, app: AppInstallState) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id: "test-host",
        display_name: "Test Host",
        profile_state,
        profile_source: None,
        profile_keys: BTreeMap::new(),
        host_running: false,
        host_processes: Vec::new(),
        app_installed: app,
        probed_at_unix: 1_700_000_000,
    }
}

fn proxy(state: ProxyProbeState) -> ProxyHealth {
    ProxyHealth {
        state,
        ..Default::default()
    }
}

const fn inputs<'a>(
    snap: Option<&'a HostAppSnapshot>,
    px: &'a ProxyHealth,
) -> HostHealthInputs<'a> {
    HostHealthInputs {
        snapshot: snap,
        proxy: px,
        models: HostModelViewRef {
            checked: true,
            available: true,
            unconfigured_providers: &[],
        },
        has_download_url: true,
        surface: AgentSurface::LocalProfile,
        manifest_synced: true,
        can_open: true,
    }
}

// The `|| "absent"` fallback this replaces classified a host that had never
// been probed as "not set up", which offered the reader an Add button for an
// agent that might already be installed.
#[test]
fn never_probed_is_checking_not_absent() {
    let px = proxy(ProxyProbeState::Listening);
    let v = verdict(&inputs(None, &px));
    assert_eq!(v.state, AgentState::Checking);
    assert!(v.action.is_none());
    assert!(!v.is_set_up);
}

// `AppInstallState` documents that `Unknown` must never render as absence.
// The JS read any non-`installed` value as missing.
#[test]
fn unknown_app_install_is_not_treated_as_missing() {
    let px = proxy(ProxyProbeState::Listening);
    let snap = snapshot(ProfileState::Installed, AppInstallState::Unknown);
    assert_eq!(
        verdict(&inputs(Some(&snap), &px)).state,
        AgentState::Working
    );
}

#[test]
fn app_missing_outranks_every_other_fault() {
    let px = proxy(ProxyProbeState::Refused);
    let snap = snapshot(ProfileState::Absent, AppInstallState::NotInstalled);
    let v = verdict(&inputs(Some(&snap), &px));
    assert_eq!(v.state, AgentState::Attention);
    assert_eq!(v.action, Some(AgentAction::Download));
}

#[test]
fn profile_faults_outrank_proxy_state() {
    let px = proxy(ProxyProbeState::Refused);

    let stale = snapshot(
        ProfileState::Stale {
            reason: StaleReason::LoopbackSecret,
        },
        AppInstallState::Installed,
    );
    let v = verdict(&inputs(Some(&stale), &px));
    assert_eq!(v.state, AgentState::Attention);
    assert_eq!(v.action, Some(AgentAction::Repair));

    let partial = snapshot(
        ProfileState::Partial {
            missing_required: vec!["base_url".to_owned()],
        },
        AppInstallState::Installed,
    );
    assert_eq!(
        verdict(&inputs(Some(&partial), &px)).state,
        AgentState::Attention
    );

    let absent = snapshot(ProfileState::Absent, AppInstallState::Installed);
    let v = verdict(&inputs(Some(&absent), &px));
    assert_eq!(v.state, AgentState::NotSetUp);
    assert_eq!(v.action, Some(AgentAction::Add));
    assert!(!v.is_set_up);
}

#[test]
fn proxy_state_decides_a_healthy_profile() {
    let snap = snapshot(ProfileState::Installed, AppInstallState::Installed);

    for (probe, expected) in [
        (ProxyProbeState::Unconfigured, AgentState::Ready),
        (ProxyProbeState::Listening, AgentState::Working),
        (ProxyProbeState::Unknown, AgentState::Checking),
        (ProxyProbeState::Refused, AgentState::Down),
        (ProxyProbeState::Timeout, AgentState::Down),
        (ProxyProbeState::HttpError, AgentState::Down),
    ] {
        let px = proxy(probe);
        assert_eq!(
            verdict(&inputs(Some(&snap), &px)).state,
            expected,
            "proxy {probe:?}"
        );
    }
}

// `host_running` is a process-table scan. It must never promote an
// unconfigured agent — the "Connected 1/2" bug, where the 1 was the agent
// that was not set up.
#[test]
fn a_running_process_does_not_make_an_unconfigured_agent_working() {
    let px = proxy(ProxyProbeState::Listening);
    let mut snap = snapshot(ProfileState::Absent, AppInstallState::Installed);
    snap.host_running = true;

    let v = verdict(&inputs(Some(&snap), &px));
    assert_eq!(v.state, AgentState::NotSetUp);
    assert!(v.is_running, "the app is open, and we still say so");

    let fleet = AgentFleetSummary::fold(std::iter::once(&v));
    assert_eq!(fleet.working, 0, "running is not working");
    assert_eq!(fleet.running, 1);
    assert_eq!(fleet.state, FleetState::Warn);
}

#[test]
fn no_usable_model_is_reported_before_proxy_health() {
    let px = proxy(ProxyProbeState::Listening);
    let snap = snapshot(ProfileState::Installed, AppInstallState::Installed);
    let providers = vec!["anthropic".to_owned()];
    let v = verdict(&HostHealthInputs {
        models: HostModelViewRef {
            checked: true,
            available: false,
            unconfigured_providers: &providers,
        },
        ..inputs(Some(&snap), &px)
    });
    assert_eq!(v.state, AgentState::Attention);
    assert!(v.action.is_none(), "no button fixes a missing API key here");
}

// A terminal-only host has nothing to bring to the foreground, so a governed
// one is reported as working with no button rather than with an Open button
// whose only outcome is an error toast.
#[test]
fn a_host_that_cannot_be_opened_gets_no_open_action() {
    let px = proxy(ProxyProbeState::Listening);
    let snap = snapshot(ProfileState::Installed, AppInstallState::Installed);
    let v = verdict(&HostHealthInputs {
        can_open: false,
        ..inputs(Some(&snap), &px)
    });
    assert_eq!(v.state, AgentState::Working);
    assert!(v.action.is_none(), "{:?}", v.action);

    let px_idle = proxy(ProxyProbeState::Unconfigured);
    let ready = verdict(&HostHealthInputs {
        can_open: false,
        ..inputs(Some(&snap), &px_idle)
    });
    assert_eq!(ready.state, AgentState::Ready);
    assert!(ready.action.is_none());
}

// A sync-only agent (claude-code, cowork) has no local profile to install, so
// it must never appear as a fault the user is asked to fix.
#[test]
fn sync_only_agents_are_cloud_managed_and_never_need_attention() {
    let px = proxy(ProxyProbeState::Refused);
    let v = verdict(&HostHealthInputs {
        surface: AgentSurface::SyncOnly,
        ..inputs(None, &px)
    });
    assert_eq!(v.state, AgentState::Working);
    assert!(v.action.is_none());
    assert!(v.is_set_up);

    let unsynced = verdict(&HostHealthInputs {
        surface: AgentSurface::SyncOnly,
        manifest_synced: false,
        ..inputs(None, &px)
    });
    assert_eq!(unsynced.state, AgentState::Checking);
}

// The property that makes the summary card trustworthy: `Ok` is reachable
// only when no row is in a bad state. The old JS derived `state` from
// `installed` while the rows derived theirs from the proxy, so the two could
// and did disagree.
#[test]
fn fleet_ok_implies_no_row_is_faulted() {
    let px_ok = proxy(ProxyProbeState::Listening);
    let px_bad = proxy(ProxyProbeState::Refused);
    let installed = snapshot(ProfileState::Installed, AppInstallState::Installed);
    let absent = snapshot(ProfileState::Absent, AppInstallState::Installed);

    let cases: Vec<Vec<AgentVerdict>> = vec![
        vec![verdict(&inputs(Some(&installed), &px_ok))],
        vec![
            verdict(&inputs(Some(&installed), &px_ok)),
            verdict(&inputs(Some(&absent), &px_ok)),
        ],
        vec![
            verdict(&inputs(Some(&installed), &px_bad)),
            verdict(&inputs(Some(&installed), &px_ok)),
        ],
        vec![verdict(&inputs(None, &px_ok))],
        vec![],
    ];

    for verdicts in cases {
        let fleet = AgentFleetSummary::fold(verdicts.iter());
        if fleet.state == FleetState::Ok {
            assert!(
                verdicts.iter().all(|v| !matches!(
                    v.state,
                    AgentState::Down | AgentState::Attention | AgentState::NotSetUp
                )),
                "fleet reported Ok while a row was faulted"
            );
            assert_eq!(fleet.headline, FleetHeadline::AllWorking);
        }
        if verdicts.iter().any(|v| v.state == AgentState::Down) {
            assert_eq!(fleet.state, FleetState::Err);
        }
    }
}

#[test]
fn empty_fleet_is_unknown_not_ok() {
    let fleet = AgentFleetSummary::fold(std::iter::empty());
    assert_eq!(fleet.state, FleetState::Unknown);
    assert_eq!(fleet.headline, FleetHeadline::NoneEnabled);
    assert_eq!(fleet.total, 0);
}

// v0.43.0 toasted "unknown host: claude-code" whenever anyone pressed a button
// on the Claude Code row. `claude-code` is a sync-only agent: listed in the
// Agents tab, but with no `HostApp`, so `find_host_by_id` cannot see it and
// every per-host handler answered NotFound. The drawer offered all five actions
// regardless, because nothing told it not to.
//
// `HostCapabilities::for_surface` is now the single answer to "what may the GUI
// offer for this agent", read by the wire payload and rendered by the drawer.
#[test]
fn sync_only_agents_offer_no_local_action() {
    let caps = HostCapabilities::for_surface(AgentSurface::SyncOnly, false);

    assert!(!caps.can_verify, "nothing local to probe");
    assert!(!caps.can_repair, "no profile to generate");
    assert!(!caps.can_open_config, "no config file on this computer");
    assert!(!caps.can_remove, "nothing installed to remove");
    assert!(!caps.can_open, "no local process to launch");
}

#[test]
fn sync_only_ignores_a_can_open_claim_from_the_host_side() {
    // Why: `can_open` is the only capability a local host answers for itself,
    // so it is the one that could leak a true through. It must not.
    let caps = HostCapabilities::for_surface(AgentSurface::SyncOnly, true);
    assert!(!caps.can_open);
}

#[test]
fn local_profile_agents_keep_every_action_and_defer_on_open() {
    for can_open in [true, false] {
        let caps = HostCapabilities::for_surface(AgentSurface::LocalProfile, can_open);
        assert!(caps.can_verify);
        assert!(caps.can_repair);
        assert!(caps.can_open_config);
        assert!(caps.can_remove);
        assert_eq!(
            caps.can_open, can_open,
            "can_open is the host's own answer — some local hosts cannot be launched"
        );
    }
}

#[test]
fn every_declared_sync_only_agent_is_covered() {
    // Table-driven off the inventory, so a sync-only agent added later is
    // covered the day it is added rather than the day it breaks.
    assert!(!SYNC_ONLY_AGENTS.is_empty());
    for agent in SYNC_ONLY_AGENTS {
        let caps = HostCapabilities::for_surface(AgentSurface::SyncOnly, false);
        assert!(
            !caps.can_verify && !caps.can_repair && !caps.can_open_config && !caps.can_remove,
            "{} must offer no local action",
            agent.id
        );
    }
}
