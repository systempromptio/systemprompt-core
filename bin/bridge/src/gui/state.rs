mod builder;
mod counters;
mod jwt;

pub use builder::AppStateSnapshotBuilder;

use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

pub use jwt::decode_jwt_identity_unverified;

use crate::auth::{cache, setup};
use crate::config::{self, paths};

use crate::gui::hosts::state::HostsState;

use crate::integration::{HostAppSnapshot, ProxyHealth};
use crate::validate::ValidationReport;

use counters::{count_malformed_plugin_dirs, count_plugin_dirs};

#[derive(Debug, Deserialize)]
struct LastSyncRecord {
    #[serde(default)]
    synced_at: Option<String>,
    #[serde(default)]
    manifest_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum GatewayStatus {
    #[default]
    Unknown,
    Probing,
    Reachable {
        latency_ms: u64,
    },
    Unreachable {
        reason: String,
    },
}

impl GatewayStatus {
    pub fn is_reachable(&self) -> bool {
        matches!(self, Self::Reachable { .. })
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedIdentity {
    pub email: Option<String>,
    pub user_id: Option<String>,
    pub tenant_id: Option<String>,
    pub exp_unix: Option<u64>,
    pub verified_at_unix: u64,
}

#[derive(Debug, Clone)]
pub struct GatewayProbeOutcome {
    pub status: GatewayStatus,
    pub identity: Option<VerifiedIdentity>,
    pub at_unix: u64,
}

#[derive(Debug, Clone, Default)]
pub struct AppStateSnapshot {
    pub gateway_url: String,
    pub config_file: String,
    pub pat_file: String,
    pub config_present: bool,
    pub pat_present: bool,
    pub last_sync_summary: Option<String>,
    pub skill_count: Option<usize>,
    pub agent_count: Option<usize>,
    pub plugins_dir: Option<String>,
    pub sync_in_flight: bool,
    pub last_validation: Option<ValidationReport>,
    pub cached_token: Option<CachedToken>,
    pub plugin_count: Option<usize>,
    pub malformed_plugin_count: Option<usize>,
    pub gateway_status: GatewayStatus,
    pub verified_identity: Option<VerifiedIdentity>,
    pub last_probe_at_unix: Option<u64>,
    pub agents_onboarded: bool,

    pub hosts: HostsState,
}

impl AppStateSnapshot {
    pub fn signed_in(&self) -> bool {
        self.gateway_status.is_reachable() && self.verified_identity.is_some()
    }

    pub fn builder() -> AppStateSnapshotBuilder {
        AppStateSnapshotBuilder::default()
    }
}

#[derive(Debug, Clone)]
pub struct CachedToken {
    pub ttl_seconds: u64,
    pub length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelScope {
    Sync,
    Login,
    GatewayProbe,
}

#[derive(Default)]
struct CancelTokens {
    sync: Option<CancellationToken>,
    login: Option<CancellationToken>,
    gateway_probe: Option<CancellationToken>,
}

pub struct AppState {
    inner: RwLock<AppStateSnapshot>,
    cancels: RwLock<CancelTokens>,
}

impl AppState {
    pub fn new_loaded() -> Arc<Self> {
        let mut snap = AppStateSnapshot::default();
        Self::reload_into(&mut snap);
        Arc::new(Self {
            inner: RwLock::new(snap),
            cancels: RwLock::new(CancelTokens::default()),
        })
    }

    pub fn install_cancel(&self, scope: CancelScope) -> CancellationToken {
        let token = CancellationToken::new();
        let mut guard = self.cancels.write();
        let slot = match scope {
            CancelScope::Sync => &mut guard.sync,
            CancelScope::Login => &mut guard.login,
            CancelScope::GatewayProbe => &mut guard.gateway_probe,
        };
        if let Some(prev) = slot.replace(token.clone()) {
            prev.cancel();
        }
        token
    }

    pub fn clear_cancel(&self, scope: CancelScope) {
        let mut guard = self.cancels.write();
        let slot = match scope {
            CancelScope::Sync => &mut guard.sync,
            CancelScope::Login => &mut guard.login,
            CancelScope::GatewayProbe => &mut guard.gateway_probe,
        };
        *slot = None;
    }

    pub fn cancel_scope(&self, scope: CancelScope) -> bool {
        let mut guard = self.cancels.write();
        let slot = match scope {
            CancelScope::Sync => &mut guard.sync,
            CancelScope::Login => &mut guard.login,
            CancelScope::GatewayProbe => &mut guard.gateway_probe,
        };
        if let Some(token) = slot.take() {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_all(&self) {
        let mut guard = self.cancels.write();
        for token in [
            guard.sync.take(),
            guard.login.take(),
            guard.gateway_probe.take(),
        ]
        .into_iter()
        .flatten()
        {
            token.cancel();
        }
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        self.inner.read().clone()
    }

    pub fn reload(&self) {
        let mut guard = self.inner.write();
        Self::reload_into(&mut guard);
    }

    pub fn set_sync_in_flight(&self, flag: bool) {
        self.inner.write().sync_in_flight = flag;
    }

    pub fn set_validation(&self, report: ValidationReport) {
        self.inner.write().last_validation = Some(report);
    }

    pub fn mark_probing(&self) {
        self.inner.write().gateway_status = GatewayStatus::Probing;
    }

    pub fn apply_probe(&self, outcome: GatewayProbeOutcome) {
        let mut guard = self.inner.write();
        guard.gateway_status = outcome.status;
        guard.verified_identity = outcome.identity;
        guard.last_probe_at_unix = Some(outcome.at_unix);
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
    }


    pub fn mark_host_probing(&self, host_id: &str) -> bool {
        let mut guard = self.inner.write();
        let entry = guard.hosts.entry(host_id);
        if entry.probe_in_flight {
            return false;
        }
        entry.probe_in_flight = true;
        true
    }


    pub fn set_last_generated_profile(&self, host_id: &str, path: String) {
        let mut guard = self.inner.write();
        guard.hosts.entry(host_id).last_generated_profile = Some(path);
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


    pub fn first_configured_proxy_url(&self) -> Option<String> {
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

    fn reload_into(snap: &mut AppStateSnapshot) {
        let cfg = config::load();
        snap.gateway_url = config::gateway_url_or_default(&cfg).to_string();

        match setup::status() {
            Ok(s) => {
                snap.config_file = s.paths.config_file.display().to_string();
                snap.pat_file = s.paths.pat_file.display().to_string();
                snap.config_present = s.config_present;
                snap.pat_present = s.pat_present;
            },
            Err(_) => {
                snap.config_file.clear();
                snap.pat_file.clear();
                snap.config_present = false;
                snap.pat_present = false;
            },
        }

        let loc = paths::org_plugins_effective();
        snap.plugins_dir = loc.as_ref().map(|l| l.path.display().to_string());
        snap.last_sync_summary = None;
        snap.skill_count = None;
        snap.agent_count = None;
        snap.plugin_count = None;
        snap.malformed_plugin_count = None;
        if crate::auth::has_credential_source(&cfg) {
            snap.cached_token = cache::read_valid().map(|out| CachedToken {
                ttl_seconds: out.ttl,
                length: out.token.len(),
            });
        } else {
            let _ = cache::clear();
            snap.cached_token = None;
            snap.verified_identity = None;
        }

        if let Some(loc) = loc {
            let meta = paths::metadata_dir(&loc.path);

            snap.plugin_count = count_plugin_dirs(&loc.path);
            snap.malformed_plugin_count = count_malformed_plugin_dirs(&loc.path);

            if let Ok(bytes) = std::fs::read(meta.join(paths::LAST_SYNC_SENTINEL))
                && let Ok(record) = serde_json::from_slice::<LastSyncRecord>(&bytes)
            {
                let when = record.synced_at.as_deref().unwrap_or("unknown");
                let manifest_version = record.manifest_version.as_deref().unwrap_or("?");
                snap.last_sync_summary = Some(format!("{when} (manifest {manifest_version})"));
            }

            let synthetic = loc.path.join(paths::SYNTHETIC_PLUGIN_NAME);
            snap.skill_count = counters::count_dir_children(&synthetic.join("skills"));
            snap.agent_count = counters::count_md_files(&synthetic.join("agents"));
        }
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
