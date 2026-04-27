use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

use crate::cache;
use crate::config;
use crate::paths;
use crate::setup;
use crate::validate::ValidationReport;

const POISONED: &str = "AppState mutex poisoned";

#[derive(Debug, Deserialize)]
struct LastSyncRecord {
    #[serde(default)]
    synced_at: Option<String>,
    #[serde(default)]
    manifest_version: Option<String>,
}

fn read_index_count(path: &std::path::Path) -> Option<usize> {
    let bytes = std::fs::read(path).ok()?;
    let entries: Vec<serde::de::IgnoredAny> = serde_json::from_slice(&bytes).ok()?;
    Some(entries.len())
}

fn count_plugin_dirs(root: &std::path::Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            n += 1;
        }
    }
    Some(n)
}

#[derive(Debug, Clone)]
pub enum GatewayStatus {
    Unknown,
    Probing,
    Reachable { latency_ms: u64 },
    Unreachable { reason: String },
}

impl Default for GatewayStatus {
    fn default() -> Self {
        Self::Unknown
    }
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
    pub last_action_message: Option<String>,
    pub last_validation: Option<ValidationReport>,
    pub cached_token: Option<CachedToken>,
    pub plugin_count: Option<usize>,
    pub gateway_status: GatewayStatus,
    pub verified_identity: Option<VerifiedIdentity>,
    pub last_probe_at_unix: Option<u64>,
}

impl AppStateSnapshot {
    pub fn signed_in(&self) -> bool {
        self.gateway_status.is_reachable() && self.verified_identity.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct CachedToken {
    pub ttl_seconds: u64,
    pub length: usize,
}

pub struct AppState {
    inner: Mutex<AppStateSnapshot>,
}

impl AppState {
    pub fn new_loaded() -> Arc<Self> {
        let mut snap = AppStateSnapshot::default();
        Self::reload_into(&mut snap);
        Arc::new(Self {
            inner: Mutex::new(snap),
        })
    }

    pub fn snapshot(&self) -> AppStateSnapshot {
        self.inner.lock().expect(POISONED).clone()
    }

    pub fn reload(&self) {
        let mut guard = self.inner.lock().expect(POISONED);
        Self::reload_into(&mut guard);
    }

    pub fn set_sync_in_flight(&self, flag: bool) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.sync_in_flight = flag;
    }

    pub fn set_message(&self, msg: impl Into<String>) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.last_action_message = Some(msg.into());
    }

    pub fn set_validation(&self, report: ValidationReport) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.last_validation = Some(report);
    }

    pub fn mark_probing(&self) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.gateway_status = GatewayStatus::Probing;
    }

    pub fn apply_probe(&self, outcome: GatewayProbeOutcome) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.gateway_status = outcome.status;
        guard.verified_identity = outcome.identity;
        guard.last_probe_at_unix = Some(outcome.at_unix);
    }

    pub fn clear_verified_identity(&self) {
        let mut guard = self.inner.lock().expect(POISONED);
        guard.verified_identity = None;
    }

    fn reload_into(snap: &mut AppStateSnapshot) {
        let cfg = config::load();
        snap.gateway_url = config::gateway_url_or_default(&cfg);

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
        snap.cached_token = cache::read_valid().map(|out| CachedToken {
            ttl_seconds: out.ttl,
            length: out.token.len(),
        });

        if let Some(loc) = loc {
            let meta = paths::metadata_dir(&loc.path);

            snap.plugin_count = count_plugin_dirs(&loc.path);

            if let Ok(bytes) = std::fs::read(meta.join(paths::LAST_SYNC_SENTINEL)) {
                if let Ok(record) = serde_json::from_slice::<LastSyncRecord>(&bytes) {
                    let when = record.synced_at.as_deref().unwrap_or("unknown");
                    let manifest_version =
                        record.manifest_version.as_deref().unwrap_or("?");
                    snap.last_sync_summary =
                        Some(format!("{when} (manifest {manifest_version})"));
                }
            }

            snap.skill_count =
                read_index_count(&meta.join(paths::SKILLS_DIR).join("index.json"));
            snap.agent_count =
                read_index_count(&meta.join(paths::AGENTS_DIR).join("index.json"));
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sub: Option<String>,
    #[serde(default)]
    tenant_id: Option<String>,
    #[serde(default)]
    exp: Option<u64>,
}

pub fn decode_jwt_identity(token: &str) -> Option<VerifiedIdentity> {
    let mut parts = token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    let claims: JwtClaims = serde_json::from_slice(&bytes).ok()?;
    Some(VerifiedIdentity {
        email: claims.email,
        user_id: claims.sub,
        tenant_id: claims.tenant_id,
        exp_unix: claims.exp,
        verified_at_unix: now_unix(),
    })
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
