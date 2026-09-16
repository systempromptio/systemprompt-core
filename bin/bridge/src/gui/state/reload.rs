//! Reloading application state from disk and configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{AppStateSnapshot, CachedToken};
use crate::auth::{cache, setup};
use crate::config::{self, paths};

use super::counters::{count_malformed_plugin_dirs, count_plugin_dirs};

pub(super) fn reload_into(snap: &mut AppStateSnapshot) {
    let cfg = match config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            snap.gateway_status = super::GatewayStatus::Unreachable {
                reason: e.to_string(),
            };
            snap.cached_token = None;
            snap.verified_identity = None;
            return;
        },
    };
    snap.gateway_url = config::gateway_url_or_default(&cfg).to_string();
    snap.gateway_configured = cfg.gateway_url.is_some();

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

    snap.elevated = process_elevated();
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
        snap.cached_token = match cache::read_for(&cfg, &gateway, 30) {
            Ok(token) => token.map(|out| CachedToken {
                ttl_seconds: out.ttl,
                length: out.token.len(),
            }),
            Err(e) => {
                snap.gateway_status = super::GatewayStatus::Unreachable {
                    reason: format!("credential cache: {e}"),
                };
                None
            },
        };
    } else {
        if let Err(e) = cache::clear() {
            snap.gateway_status = super::GatewayStatus::Unreachable {
                reason: format!("clear credential cache: {e}"),
            };
        }
        snap.cached_token = None;
        snap.verified_identity = None;
    }

    let gateway = config::gateway_url_or_default(&cfg);
    if let Some(meta) = paths::bridge_metadata_dir() {
        match crate::last_sync::read_last_sync(&meta.join(paths::LAST_SYNC_SENTINEL)) {
            Ok(Some(record)) if record.belongs_to(&gateway) => {
                snap.last_sync_summary = Some(record.summary_line());
                snap.enabled_hosts = record.enabled_hosts;
                snap.host_model_protocols = record.host_model_protocols;
            },
            Ok(_) => {},
            Err(e) => {
                snap.last_sync_summary = Some(format!("unreadable: {e}"));
            },
        }
    }

    if let Some(loc) = loc {
        snap.plugin_count = count_plugin_dirs(&loc.path);
        snap.malformed_plugin_count = count_malformed_plugin_dirs(&loc.path);
        snap.skill_count = super::counters::count_skills_across_plugins(&loc.path);
        snap.agent_count = super::counters::count_agents_across_plugins(&loc.path);
    }
}

#[cfg(target_os = "windows")]
fn process_elevated() -> bool {
    crate::winproc::is_elevated()
}

#[cfg(not(target_os = "windows"))]
const fn process_elevated() -> bool {
    false
}
