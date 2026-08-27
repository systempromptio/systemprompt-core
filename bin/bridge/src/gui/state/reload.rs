//! Reloading application state from disk and configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;

use super::{AppStateSnapshot, CachedToken};
use crate::auth::{cache, setup};
use crate::config::{self, paths};

use super::counters::{count_malformed_plugin_dirs, count_plugin_dirs};

#[derive(Debug, Deserialize)]
struct LastSyncRecord {
    #[serde(default)]
    synced_at: Option<String>,
    #[serde(default)]
    manifest_version: Option<String>,
    #[serde(default)]
    enabled_hosts: Vec<String>,
    #[serde(default)]
    host_model_protocols: std::collections::BTreeMap<String, Vec<String>>,
}

pub(super) fn reload_into(snap: &mut AppStateSnapshot) {
    let cfg = config::load();
    snap.gateway_url = config::gateway_url_or_default(&cfg).to_string();

    snap.first_run.done = crate::gui::first_run::record::read().is_some();
    snap.agents_onboarded = crate::gui::onboarding::is_complete();

    if let Ok(s) = setup::status() {
        snap.config_file = s.paths.config_file.display().to_string();
        snap.pat_file = s.paths.pat_file.display().to_string();
        snap.config_present = s.config_present;
        snap.pat_present = s.pat_present;
    } else {
        snap.config_file.clear();
        snap.pat_file.clear();
        snap.config_present = false;
        snap.pat_present = false;
    }

    let loc = paths::org_plugins_effective();
    snap.plugins_dir = loc.as_ref().map(|l| l.path.display().to_string());
    snap.last_sync_summary = None;
    snap.skill_count = None;
    snap.agent_count = None;
    snap.plugin_count = None;
    snap.malformed_plugin_count = None;
    snap.enabled_hosts.clear();
    snap.host_model_protocols.clear();
    if crate::auth::has_credential_source(&cfg) {
        let gateway = config::gateway_url_or_default(&cfg);
        snap.cached_token = cache::read_valid(&gateway).map(|out| CachedToken {
            ttl_seconds: out.ttl,
            length: out.token.len(),
        });
    } else {
        _ = cache::clear();
        snap.cached_token = None;
        snap.verified_identity = None;
    }

    // Why: the last-sync record is the manifest's own footprint, not the
    // org-plugins directory's, so it must be read even when that directory
    // does not resolve or the host gate silently loses its authority.
    if let Some(meta) = paths::bridge_metadata_dir()
        && let Ok(bytes) = std::fs::read(meta.join(paths::LAST_SYNC_SENTINEL))
        && let Ok(record) = serde_json::from_slice::<LastSyncRecord>(&bytes)
    {
        let when = record.synced_at.as_deref().unwrap_or("unknown");
        let manifest_version = record.manifest_version.as_deref().unwrap_or("?");
        snap.last_sync_summary = Some(format!("{when} (manifest {manifest_version})"));
        snap.enabled_hosts = record.enabled_hosts;
        snap.host_model_protocols = record.host_model_protocols;
    }

    if let Some(loc) = loc {
        snap.plugin_count = count_plugin_dirs(&loc.path);
        snap.malformed_plugin_count = count_malformed_plugin_dirs(&loc.path);
        snap.skill_count = super::counters::count_skills_across_plugins(&loc.path);
        snap.agent_count = super::counters::count_agents_across_plugins(&loc.path);
    }
}
