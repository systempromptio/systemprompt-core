//! Host probe results are applied in issue order: a result from a probe that
//! a later probe of the same host superseded is discarded, so two overlapping
//! probes (a manual re-verify racing the periodic tick) cannot land out of
//! order and leave the older snapshot on screen.

use std::collections::BTreeMap;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::gui::state::AppState;
use systemprompt_bridge::integration::{AppInstallState, HostAppSnapshot, ProfileState};
use systemprompt_models::bridge::host::HostKind;

fn state() -> std::sync::Arc<AppState> {
    let ctx = BridgeContext::start(ProxyMode::Attach).expect("runtime builds");
    AppState::new_loaded(ctx)
}

fn snapshot(probed_at_unix: u64) -> HostAppSnapshot {
    HostAppSnapshot {
        host_id: HostKind::CodexCli,
        display_name: "Codex CLI",
        profile_state: ProfileState::Installed,
        profile_source: None,
        profile_keys: BTreeMap::new(),
        probe_error: None,
        host_running: Some(true),
        host_processes: Vec::new(),
        app_installed: AppInstallState::Installed,
        probed_at_unix,
        update_needs_approval: false,
        declared_models: None,
    }
}

#[test]
fn a_superseded_probe_result_is_discarded_and_the_newest_wins() {
    let state = state();
    let first = state
        .begin_host_probe(HostKind::CodexCli, true)
        .expect("the first exclusive probe is issued");
    assert!(
        state.begin_host_probe(HostKind::CodexCli, true).is_none(),
        "an exclusive (tick) probe stands down while one is in flight"
    );
    let second = state
        .begin_host_probe(HostKind::CodexCli, false)
        .expect("a manual probe is issued even while one is in flight");
    assert!(first < second);

    assert!(
        state.apply_host_snapshot(HostKind::CodexCli, second, snapshot(2)),
        "the newest probe's result is applied"
    );
    assert!(
        !state.apply_host_snapshot(HostKind::CodexCli, first, snapshot(1)),
        "the older probe's late result is discarded"
    );
    let recorded = state
        .snapshot()
        .hosts
        .get(HostKind::CodexCli)
        .and_then(|h| h.snapshot.as_ref().map(|s| s.probed_at_unix));
    assert_eq!(
        recorded,
        Some(2),
        "the older result did not overwrite the newer"
    );
    assert!(
        state.begin_host_probe(HostKind::CodexCli, true).is_some(),
        "the host is no longer marked in flight once the newest result landed"
    );
}

#[test]
fn a_failed_older_probe_does_not_clear_the_in_flight_mark_of_a_newer_one() {
    let state = state();
    let first = state
        .begin_host_probe(HostKind::Hermes, true)
        .expect("issued");
    let second = state
        .begin_host_probe(HostKind::Hermes, false)
        .expect("issued");

    state.finish_failed_probe(Some((HostKind::Hermes, first)));
    assert!(
        state.begin_host_probe(HostKind::Hermes, true).is_none(),
        "the newer probe is still in flight"
    );

    state.finish_failed_probe(Some((HostKind::Hermes, second)));
    assert!(
        state.begin_host_probe(HostKind::Hermes, true).is_some(),
        "the newest probe's failure releases the host"
    );
}
