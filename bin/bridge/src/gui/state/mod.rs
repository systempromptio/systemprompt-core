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

pub use builder::AppStateSnapshotBuilder;
pub use cancel::CancelScope;
pub use jwt::decode_jwt_identity_unverified;
pub use types::{
    AppStateSnapshot, CachedToken, GatewayProbeOutcome, GatewayStatus, VerifiedIdentity,
};

use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::integration::{HostAppSnapshot, ProxyHealth};
use crate::proxy::mcp_probe::McpServerAuth;
use crate::validate::ValidationReport;
use cancel::CancelTokens;

#[derive(Debug)]
pub struct AppState {
    inner: RwLock<AppStateSnapshot>,
    cancels: RwLock<CancelTokens>,
    // Why: `Probing` overwrites the field that holds the last conclusive
    // answer. A probe that never concludes has to put that answer back rather
    // than leave the UI stuck on a transient state.
    pre_probe_status: RwLock<Option<GatewayStatus>>,
}

impl AppState {
    pub fn new_loaded() -> Arc<Self> {
        let mut snap = AppStateSnapshot::default();
        reload::reload_into(&mut snap);
        Arc::new(Self {
            inner: RwLock::new(snap),
            cancels: RwLock::new(CancelTokens::default()),
            pre_probe_status: RwLock::new(None),
        })
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        self.inner.read().clone()
    }

    pub fn reload(&self) {
        let mut guard = self.inner.write();
        reload::reload_into(&mut guard);
    }

    pub fn set_sync_in_flight(&self, flag: bool) {
        self.inner.write().sync_in_flight = flag;
    }

    pub fn set_validation(&self, report: ValidationReport) {
        let mut guard = self.inner.write();
        guard.last_validation = Some(report);
        guard.last_validation_at_unix = Some(now_unix());
    }

    pub fn set_last_sync_report(&self, summary: crate::sync::SyncSummary) {
        self.inner.write().last_sync_report = Some(summary);
    }

    pub fn mark_probing(&self) {
        let mut guard = self.inner.write();
        let prior = std::mem::replace(&mut guard.gateway_status, GatewayStatus::Probing);
        drop(guard);
        if !matches!(prior, GatewayStatus::Probing) {
            *self.pre_probe_status.write() = Some(prior);
        }
    }

    // Why: Put back the status this probe replaced, for a probe that concluded
    // nothing. Identity and provider health were never touched, so there is
    // nothing else to undo.
    pub fn abandon_probe(&self) {
        let prior = self.pre_probe_status.write().take();
        if let Some(prior) = prior {
            self.inner.write().gateway_status = prior;
        }
    }

    pub fn apply_probe(&self, outcome: GatewayProbeOutcome) {
        *self.pre_probe_status.write() = None;
        let mut guard = self.inner.write();
        let reachable = outcome.status.is_reachable();
        guard.gateway_status = outcome.status;
        guard.last_probe_at_unix = Some(outcome.at_unix);
        // Why: an unreachable gateway cannot tell us who we are, so the probe
        // returns no identity and no provider list. Writing those empties in
        // would report a transient network fault as a sign-out and blank the
        // provider panel. The last verified answer stands, timestamped by its
        // own `verified_at_unix`, until a reachable probe replaces it or the
        // user signs out (`clear_verified_identity`).
        if reachable {
            guard.verified_identity = outcome.identity;
            guard.provider_health = outcome.provider_health;
        }
    }

    pub fn gateway_probe_in_flight(&self) -> bool {
        self.has_cancel(CancelScope::GatewayProbe)
    }

    pub fn clear_verified_identity(&self) {
        self.inner.write().verified_identity = None;
    }

    pub fn set_agents_onboarded(&self, flag: bool) {
        self.inner.write().agents_onboarded = flag;
    }

    pub fn apply_host_snapshot(&self, host_id: &str, snap: HostAppSnapshot) {
        let mut guard = self.inner.write();
        let entry = guard.hosts.entry(host_id);
        entry.snapshot = Some(snap);
        entry.probe_in_flight = false;
        drop(guard);
    }


    pub fn mark_host_probing(&self, host_id: &str) -> bool {
        let mut guard = self.inner.write();
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
        let mut guard = self.inner.write();
        guard.hosts.entry(host_id).last_generated_profile = Some(profile);
    }


    pub fn mark_proxy_probing(&self) -> bool {
        let mut guard = self.inner.write();
        if guard.hosts.proxy_probe_in_flight {
            return false;
        }
        guard.hosts.proxy_probe_in_flight = true;
        true
    }


    pub fn apply_proxy_health(&self, health: ProxyHealth) {
        let mut guard = self.inner.write();
        guard.hosts.local_proxy = health;
        guard.hosts.proxy_probe_in_flight = false;
    }


    pub fn mark_mcp_auth_probing(&self) -> bool {
        let mut guard = self.inner.write();
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
        let mut guard = self.inner.write();
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


    pub fn set_update_state(&self, state: crate::update::UpdateUiState) {
        let mut guard = self.inner.write();
        guard.update = state;
    }

    pub fn set_update_progress(&self, version: &str, percent: u8) {
        let mut guard = self.inner.write();
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
        let guard = self.inner.read();
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
