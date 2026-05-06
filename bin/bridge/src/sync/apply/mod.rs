mod error;
mod hooks;
pub(crate) mod hooks_schema;
mod plugin;
mod synthetic_plugin;

pub use error::ApplyError;
pub use synthetic_plugin::write_synthetic_plugin;

use crate::config::paths::{self, OrgPluginsLocation};
use crate::config::{self as config};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, UserInfo};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::ValidatedUrl;
use url::{Host, Url};

pub use plugin::PluginApplyOutcome as ApplyReport;

pub async fn apply_manifest(
    client: &GatewayClient,
    bearer: &str,
    manifest: &SignedManifest,
    location: &OrgPluginsLocation,
) -> Result<ApplyReport, ApplyError> {
    let root = &location.path;
    let (meta_dir, staging_root) = prepare_dirs(root)?;

    if let Some(reserved) = manifest
        .plugins
        .iter()
        .find(|p| p.id.as_str() == paths::SYNTHETIC_PLUGIN_NAME)
    {
        return Err(ApplyError::ReservedPluginId(reserved.id.clone()));
    }

    let report = plugin::apply_plugins(client, bearer, manifest, root, &staging_root).await?;

    let _ = fs::remove_dir_all(&staging_root);

    let mcp_servers = rewrite_loopback_urls(&manifest.managed_mcp_servers);
    let manifest_for_write = manifest_with_servers(manifest, mcp_servers.clone());
    synthetic_plugin::write_synthetic_plugin(root, &manifest_for_write)?;
    write_user(&meta_dir, manifest.user.as_ref())?;

    crate::mcp_registry::publish(&mcp_servers);

    refresh_mdm_managed_mcp();
    emit_to_cowork(root, &manifest_for_write);

    Ok(report)
}

// Best-effort: errors log but never fail the sync — MDM refresh is not on
// the critical path for plugin install.
fn refresh_mdm_managed_mcp() {
    match crate::install::refresh_managed_mcp_servers() {
        Ok(line) => tracing::info!(
            target: "bridge::mdm",
            written = %line,
            "managedMcpServers policy value refreshed"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::mdm",
            error = %e,
            "managedMcpServers policy refresh failed (non-fatal)"
        ),
    }
}

fn emit_to_cowork(org_plugins_root: &Path, manifest: &SignedManifest) {
    if std::env::var("SP_BRIDGE_NO_COWORK_EMIT").is_ok() {
        tracing::info!(
            target: "bridge::cowork",
            "SP_BRIDGE_NO_COWORK_EMIT set; skipping Cowork marketplace emit"
        );
        return;
    }
    let Some(target) = crate::integration::cowork_plugins::resolve_target() else {
        tracing::info!(
            target: "bridge::cowork",
            "no Cowork install detected; skipping marketplace emit"
        );
        return;
    };
    let plugin_name = paths::SYNTHETIC_PLUGIN_NAME;
    let version = manifest.manifest_version.as_str();
    match crate::integration::cowork_plugins::publish(
        &target,
        org_plugins_root,
        plugin_name,
        version,
        Some("Skills, agents, and MCP servers managed by your organization."),
    ) {
        Ok(report) => tracing::info!(
            target: "bridge::cowork",
            session_org = ?report.target,
            copied = report.plugin_copied,
            registered_marketplace = report.marketplace_registered,
            registered_plugin = report.plugin_installed_registered,
            enabled = report.enabled,
            "Cowork marketplace emit complete"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::cowork",
            error = %e,
            "Cowork marketplace emit failed (non-fatal)"
        ),
    }
}

// Manifests can carry loopback URLs (the gateway encodes its own
// `gateway_url` at emit time); a Cowork client on a different host cannot
// reach those. Substitute the bridge's configured gateway host.
fn rewrite_loopback_urls(servers: &[ManagedMcpServer]) -> Vec<ManagedMcpServer> {
    let cfg = config::load();
    let Some(gateway) = cfg.gateway_url.as_ref() else {
        return servers.to_vec();
    };
    let Ok(gateway_url) = Url::parse(gateway.as_str()) else {
        return servers.to_vec();
    };
    let (Some(gw_host), gw_scheme) = (gateway_url.host_str(), gateway_url.scheme()) else {
        return servers.to_vec();
    };
    let gw_port = gateway_url.port();
    servers
        .iter()
        .map(|s| {
            let url_str = s.url.as_str();
            let Ok(mut parsed) = Url::parse(url_str) else {
                return s.clone();
            };
            let is_loopback = match parsed.host() {
                Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
                Some(Host::Ipv4(addr)) => addr.is_loopback(),
                Some(Host::Ipv6(addr)) => addr.is_loopback(),
                None => false,
            };
            if !is_loopback {
                return s.clone();
            }
            if parsed.set_scheme(gw_scheme).is_err() {
                return s.clone();
            }
            if parsed.set_host(Some(gw_host)).is_err() {
                return s.clone();
            }
            // Why: set_port returns Err for cannot-be-a-base URLs; for http(s) this only fails on
            // truly invalid input. Mirror gateway port (None clears the explicit port).
            if parsed.set_port(gw_port).is_err() {
                return s.clone();
            }
            let rebuilt = parsed.to_string();
            match ValidatedUrl::try_new(&rebuilt) {
                Ok(url) => {
                    tracing::info!(
                        target: "bridge::sync",
                        original = %url_str,
                        rewritten = %rebuilt,
                        "rewrote loopback MCP URL to gateway host"
                    );
                    let mut next = s.clone();
                    next.url = url;
                    next
                },
                Err(e) => {
                    tracing::warn!(
                        target: "bridge::sync",
                        original = %url_str,
                        rewritten = %rebuilt,
                        error = %e,
                        "loopback rewrite produced invalid URL; keeping original"
                    );
                    s.clone()
                },
            }
        })
        .collect()
}

fn manifest_with_servers(base: &SignedManifest, servers: Vec<ManagedMcpServer>) -> SignedManifest {
    let mut next = base.clone();
    next.managed_mcp_servers = servers;
    next
}

fn prepare_dirs(root: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf), ApplyError> {
    fs::create_dir_all(root).map_err(|e| ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;
    let meta_dir = paths::metadata_dir(root);
    fs::create_dir_all(&meta_dir).map_err(|e| ApplyError::Io {
        context: "create metadata dir".into(),
        source: e,
    })?;
    let staging_root = paths::staging_dir(root);
    let _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| ApplyError::Io {
        context: "create staging".into(),
        source: e,
    })?;
    Ok((meta_dir, staging_root))
}

fn write_user(meta_dir: &Path, user: Option<&UserInfo>) -> Result<(), ApplyError> {
    let path = meta_dir.join(paths::USER_FRAGMENT);
    let bytes = match user {
        Some(u) => serde_json::to_vec_pretty(u).map_err(|e| ApplyError::Serialize {
            what: "user".into(),
            source: e,
        })?,
        None => b"null".to_vec(),
    };
    fs::write(&path, bytes).map_err(|e| ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}
