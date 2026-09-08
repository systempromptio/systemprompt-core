//! Self-diagnosis checks: binary, org-plugins tree, and last-sync state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod policy;
mod report;

use crate::auth::cache;
use crate::config;
use crate::config::paths::{self, Scope};
use crate::gateway::GatewayClient;

use self::report::Report;
pub use self::report::{CheckLevel, CheckLine, ValidationCode, ValidationReport};

pub async fn run(http: &reqwest::Client) -> ValidationReport {
    let mut report = Report::new();
    check_binary(&mut report);
    check_org_plugins(&mut report);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    policy::check_managed_policy(&mut report);
    check_gateway(&mut report, http).await;
    check_cached_token(&mut report);
    check_pinned_pubkey(&mut report);
    report.into_report()
}

fn check_binary(report: &mut Report) {
    report.info(
        "binary",
        &format!(
            "{} v{} ({}-{})",
            crate::brand::brand().binary_name,
            crate::brand::brand().version,
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
    );
}

fn check_org_plugins(report: &mut Report) {
    let Some(loc) = paths::org_plugins_effective() else {
        report.fail("org-plugins path", "unresolvable for this OS");
        return;
    };
    let scope = match loc.scope {
        Scope::System => "system",
        Scope::User => "user",
    };
    report.ok(
        "org-plugins path",
        &format!("{} (scope: {scope})", loc.path.display()),
    );

    let Some(meta) = paths::bridge_metadata_dir() else {
        report.warn("metadata dir", "bridge metadata dir unresolvable");
        return;
    };
    if meta.exists() {
        report.ok("metadata dir", &meta.display().to_string());
    } else {
        report.warn(
            "metadata dir",
            &format!("{} (missing — run `install`)", meta.display()),
        );
    }

    check_last_sync(report, &meta);

    match count_installed_plugins(&loc.path) {
        Some(n) => report.ok("plugins on disk", &format!("{n}")),
        None => report.warn("plugins on disk", "could not enumerate"),
    }
}

fn check_last_sync(report: &mut Report, meta: &std::path::Path) {
    let last_sync = meta.join(paths::LAST_SYNC_SENTINEL);
    if !last_sync.exists() {
        report.warn("last sync", "never — run `sync`");
        return;
    }
    match std::fs::read_to_string(&last_sync) {
        Ok(s) => report.ok("last sync", &summarise_last_sync(&s)),
        Err(e) => report.warn("last sync", &format!("unreadable: {e}")),
    }
}

async fn check_gateway(report: &mut Report, http: &reqwest::Client) {
    let Some(cfg) = loaded_config(report) else {
        return;
    };
    let Some(url) = cfg.gateway_url.as_ref() else {
        report.fail("gateway_url", "not set in config");
        return;
    };
    report.ok("gateway_url", url.as_str());
    let client = GatewayClient::new(url.clone(), http.clone());
    match client.health().await {
        Ok(()) => report.ok("gateway /health", "reachable"),
        Err(e) => report.fail("gateway /health", &e.to_string()),
    }
}

fn loaded_config(report: &mut Report) -> Option<config::Config> {
    match config::load() {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            report.fail("config", &e.to_string());
            None
        },
    }
}

fn check_cached_token(report: &mut Report) {
    let Some(cfg) = loaded_config(report) else {
        return;
    };
    let gateway = config::gateway_url_or_default(&cfg);
    match cache::read_for(&cfg, &gateway, 30) {
        Err(e) => report.fail("cached token", &e.to_string()),
        Ok(Some(out)) => report.ok(
            "cached token",
            &format!("ttl={}s, len={}", out.ttl, out.token.len()),
        ),
        Ok(None) => report.warn(
            "cached token",
            "absent, expired, or minted for another gateway — helper will probe \
             providers on next run",
        ),
    }
}

fn check_pinned_pubkey(report: &mut Report) {
    match config::pinned_pubkey_state() {
        Err(e) => report.fail("manifest pubkey", &e.to_string()),
        Ok(config::PinnedPubkeyState::Pinned { key, source }) => report.ok(
            "pinned manifest pubkey",
            &format!("{} chars, from the {}", key.as_str().len(), source.label()),
        ),
        Ok(config::PinnedPubkeyState::StaleForGateway {
            pinned_for,
            current,
        }) => report.fail(
            "pinned manifest pubkey",
            &format!(
                "pinned for {pinned_for} but the gateway is {current} — the pin is not in effect; \
                 explicitly pin the key with `install --apply --pubkey <base64>`"
            ),
        ),
        Ok(config::PinnedPubkeyState::Unpinned) => report.fail(
            "pinned manifest pubkey",
            "not pinned — provide it out of band via MDM (`install --apply --pubkey <base64>`) or \
             rerun `sync --allow-tofu`",
        ),
    }
}

pub fn summarise_last_sync(raw: &str) -> String {
    #[derive(serde::Deserialize)]
    struct LastSyncRecord {
        #[serde(default)]
        synced_at: Option<String>,
        #[serde(default)]
        manifest_version: Option<String>,
        #[serde(default)]
        mcp_server_count: Option<u64>,
    }

    let Ok(record) = serde_json::from_str::<LastSyncRecord>(raw) else {
        return "unparseable".into();
    };
    let synced_at = record.synced_at.as_deref().unwrap_or("unknown");
    let manifest_version = record.manifest_version.as_deref().unwrap_or("?");
    let mcp_count = record.mcp_server_count.unwrap_or(0);
    format!("{synced_at} (manifest {manifest_version}, {mcp_count} MCP server(s))")
}

pub fn count_installed_plugins(org_plugins: &std::path::Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(org_plugins).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok()?.is_dir() {
            n += 1;
        }
    }
    Some(n)
}
