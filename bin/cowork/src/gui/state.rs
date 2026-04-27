use std::sync::{Arc, Mutex};

use crate::cache;
use crate::config;
use crate::paths;
use crate::setup;
use crate::validate::ValidationReport;

#[derive(Debug, Clone, Default)]
pub struct AppStateSnapshot {
    pub identity: Option<String>,
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
        self.inner.lock().unwrap().clone()
    }

    pub fn reload(&self) {
        let mut guard = self.inner.lock().unwrap();
        Self::reload_into(&mut guard);
    }

    pub fn set_sync_in_flight(&self, flag: bool) {
        let mut guard = self.inner.lock().unwrap();
        guard.sync_in_flight = flag;
    }

    pub fn set_message(&self, msg: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap();
        guard.last_action_message = Some(msg.into());
    }

    pub fn set_validation(&self, report: ValidationReport) {
        let mut guard = self.inner.lock().unwrap();
        guard.last_validation = Some(report);
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
        snap.identity = None;
        snap.last_sync_summary = None;
        snap.skill_count = None;
        snap.agent_count = None;
        snap.cached_token = cache::read_valid().map(|out| CachedToken {
            ttl_seconds: out.ttl,
            length: out.token.len(),
        });

        if let Some(loc) = loc {
            let meta = paths::metadata_dir(&loc.path);

            let user_file = meta.join(paths::USER_FRAGMENT);
            if let Ok(bytes) = std::fs::read(&user_file) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    snap.identity = value
                        .get("email")
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                }
            }

            let last_sync_path = meta.join(paths::LAST_SYNC_SENTINEL);
            if let Ok(bytes) = std::fs::read(&last_sync_path) {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    let when = value
                        .get("synced_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let mv = value
                        .get("manifest_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    snap.last_sync_summary = Some(format!("{when} (manifest {mv})"));
                }
            }

            let skills_idx = meta.join(paths::SKILLS_DIR).join("index.json");
            if let Ok(bytes) = std::fs::read(&skills_idx) {
                if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    snap.skill_count = arr.as_array().map(|a| a.len());
                }
            }

            let agents_idx = meta.join(paths::AGENTS_DIR).join("index.json");
            if let Ok(bytes) = std::fs::read(&agents_idx) {
                if let Ok(arr) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    snap.agent_count = arr.as_array().map(|a| a.len());
                }
            }
        }
    }
}
