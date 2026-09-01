//! Application-state snapshot: auth, sync, proxy, and host status for the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod builder;
mod cancel;
mod counters;
mod first_run;
mod jwt;
mod reload;
mod types;
mod verdicts;

pub use builder::AppStateSnapshotBuilder;
pub use cancel::CancelScope;
pub use jwt::decode_jwt_identity_unverified;
pub use types::{
    AppStateSnapshot, CachedToken, GatewayProbeOutcome, GatewayStatus, VerifiedIdentity,
};
pub use verdicts::{GatewayCode, HealthCode, IdentityCode, OverallCode, TokenCode};

use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::integration::{HostAppSnapshot, ProxyHealth};
use crate::proxy::mcp_probe::McpServerAuth;
use crate::validate::ValidationReport;
use cancel::CancelTokens;

// Why: one lock, not three. The snapshot, the cancel tokens and the saved
// pre-probe status used to sit behind separate locks, and `mark_probing`
// released the first before taking the third — two probes interleaving there
// could strand the UI on `Probing` for good.
#[derive(Debug)]
struct Inner {
    snapshot: AppStateSnapshot,
    cancels: CancelTokens,
    // Why: `Probing` overwrites the field that holds the last conclusive
    // answer. A probe that never concludes has to put that answer back rather
    // than leave the UI stuck on a transient state.
    pre_probe_status: Option<GatewayStatus>,
}

#[derive(Debug)]
pub struct AppState {
    inner: RwLock<Inner>,
}

impl AppState {
    pub fn new_loaded() -> Arc<Self> {
        let mut snap = AppStateSnapshot::default();
        reload::reload_into(&mut snap);
        Arc::new(Self {
            inner: RwLock::new(Inner {
                snapshot: snap,
                cancels: CancelTokens::default(),
                pre_probe_status: None,
            }),
        })
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        self.inner.read().snapshot.clone()
    }

    fn snap_mut(&self) -> parking_lot::MappedRwLockWriteGuard<'_, AppStateSnapshot> {
        parking_lot::RwLockWriteGuard::map(self.inner.write(), |i| &mut i.snapshot)
    }

    fn cancels_mut(&self) -> parking_lot::MappedRwLockWriteGuard<'_, CancelTokens> {
        parking_lot::RwLockWriteGuard::map(self.inner.write(), |i| &mut i.cancels)
    }

    fn cancels(&self) -> parking_lot::MappedRwLockReadGuard<'_, CancelTokens> {
        parking_lot::RwLockReadGuard::map(self.inner.read(), |i| &i.cancels)
    }

    pub fn reload(&self) {
        let mut guard = self.snap_mut();
        reload::reload_into(&mut guard);
    }

    pub fn set_sync_in_flight(&self, flag: bool) {
        self.snap_mut().sync_in_flight = flag;
    }

    pub fn set_validation(&self, report: ValidationReport) {
        let mut guard = self.snap_mut();
        guard.last_validation = Some(report);
        guard.last_validation_at_unix = Some(now_unix());
    }

    pub fn set_last_sync_report(&self, summary: crate::sync::SyncSummary) {
        self.snap_mut().last_sync_report = Some(summary);
    }

    pub fn mark_probing(&self) {
        let mut guard = self.inner.write();
        let prior = std::mem::replace(&mut guard.snapshot.gateway_status, GatewayStatus::Probing);
        if !matches!(prior, GatewayStatus::Probing) {
            guard.pre_probe_status = Some(prior);
        }
    }

    // Why: Put back the status this probe replaced, for a probe that concluded
    // nothing. Identity and provider health were never touched, so there is
    // nothing else to undo.
    pub fn abandon_probe(&self) {
        let mut guard = self.inner.write();
        if let Some(prior) = guard.pre_probe_status.take() {
            guard.snapshot.gateway_status = prior;
        }
    }

    pub fn apply_probe(&self, outcome: GatewayProbeOutcome) {
        let mut inner = self.inner.write();
        inner.pre_probe_status = None;
        let reachable = outcome.status.is_reachable();
        inner.snapshot.gateway_status = outcome.status;
        inner.snapshot.last_probe_at_unix = Some(outcome.at_unix);
        // Why: an unreachable gateway cannot tell us who we are, so the probe
        // returns no identity and no provider list. Writing those empties in
        // would report a transient network fault as a sign-out and blank the
        // provider panel. The last verified answer stands, timestamped by its
        // own `verified_at_unix`, until a reachable probe replaces it or the
        // user signs out (`clear_verified_identity`).
        if reachable {
            inner.snapshot.verified_identity = outcome.identity;
            inner.snapshot.provider_health = outcome.provider_health;
        }
    }

    pub fn gateway_probe_in_flight(&self) -> bool {
        self.has_cancel(CancelScope::GatewayProbe)
    }

    pub fn clear_verified_identity(&self) {
        self.snap_mut().verified_identity = None;
    }

    pub fn set_agents_onboarded(&self, flag: bool) {
        self.snap_mut().agents_onboarded = flag;
    }

    pub fn apply_host_snapshot(&self, host_id: &str, snap: HostAppSnapshot) {
        let mut guard = self.snap_mut();
        let entry = guard.hosts.entry(host_id);
        entry.snapshot = Some(snap);
        entry.probe_in_flight = false;
        drop(guard);
    }


    pub fn mark_host_probing(&self, host_id: &str) -> bool {
        let mut guard = self.snap_mut();
        let entry = guard.hosts.entry(host_id);
        if entry.probe_in_flight {
            return false;
        }
        entry.probe_in_flight = true;
        drop(guard);
        true
    }


    pub fn set_last_generated_profile(
        &self,
        host_id: &str,
        profile: crate::integration::GeneratedProfile,
    ) {
        let mut guard = self.snap_mut();
        guard.hosts.entry(host_id).last_generated_profile = Some(profile);
    }


    pub fn mark_proxy_probing(&self) -> bool {
        let mut guard = self.snap_mut();
        if guard.hosts.proxy_probe_in_flight {
            return false;
        }
        guard.hosts.proxy_probe_in_flight = true;
        true
    }


    pub fn apply_proxy_health(&self, health: ProxyHealth) {
        let mut guard = self.snap_mut();
        guard.hosts.local_proxy = health;
        guard.hosts.proxy_probe_in_flight = false;
    }


    pub fn mark_mcp_auth_probing(&self) -> bool {
        let mut guard = self.snap_mut();
        if guard.mcp_auth_probe_in_flight {
            return false;
        }
        guard.mcp_auth_probe_in_flight = true;
        true
    }


    // Why: Apply a probe pass, keeping the last conclusive answer for any server
    // this pass could not reach.
    //
    // Why merge rather than replace: the probe has a six-second budget and
    // funnels every transport fault into a state of its own. Replacing
    // wholesale meant one slow round trip erased `Authenticated` for a server
    // that was working, which the UI then reported as needing a sign-in.
    pub fn apply_mcp_auth(&self, results: Vec<McpServerAuth>) {
        let mut guard = self.snap_mut();
        let merged = results
            .into_iter()
            .map(|fresh| {
                if fresh.state.is_conclusive() {
                    return fresh;
                }
                guard
                    .mcp_auth
                    .iter()
                    .find(|prior| prior.id == fresh.id && prior.state.is_conclusive())
                    .cloned()
                    .unwrap_or(fresh)
            })
            .collect();
        guard.mcp_auth = merged;
        guard.mcp_auth_probe_in_flight = false;
    }

    // Why: a single-server re-check replaces that server's row and leaves the
    // rest as they were; an inconclusive answer keeps the prior conclusive one
    // for the same reason `apply_mcp_auth` does.
    pub fn apply_mcp_auth_one(&self, fresh: McpServerAuth) {
        let mut guard = self.snap_mut();
        let keep_prior = !fresh.state.is_conclusive()
            && guard
                .mcp_auth
                .iter()
                .any(|prior| prior.id == fresh.id && prior.state.is_conclusive());
        if !keep_prior {
            match guard.mcp_auth.iter_mut().find(|s| s.id == fresh.id) {
                Some(slot) => *slot = fresh,
                None => guard.mcp_auth.push(fresh),
            }
        }
        guard.mcp_auth_probe_in_flight = false;
    }


    pub fn set_update_state(&self, state: crate::update::UpdateUiState) {
        let mut guard = self.snap_mut();
        guard.update = state;
    }

    pub fn set_update_progress(&self, version: &str, percent: u8) {
        let mut guard = self.snap_mut();
        if matches!(
            guard.update,
            crate::update::UpdateUiState::Available { .. }
                | crate::update::UpdateUiState::Downloading { .. }
        ) && guard.update.version() == Some(version)
        {
            guard.update = crate::update::UpdateUiState::Downloading {
                version: version.to_owned(),
                percent,
            };
        }
    }


    pub fn first_configured_proxy_url(&self) -> Option<String> {
        if crate::proxy::handle().is_some() {
            return Some(crate::proxy::loopback_origin());
        }
        let guard = parking_lot::RwLockReadGuard::map(self.inner.read(), |i| &i.snapshot);
        guard
            .hosts
            .by_id
            .values()
            .filter_map(|h| h.snapshot.as_ref())
            .find_map(|s| {
                s.profile_keys
                    .get("inferenceGatewayBaseUrl")
                    .filter(|s| !s.is_empty())
                    .cloned()
            })
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
