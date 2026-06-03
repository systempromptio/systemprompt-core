mod error;
mod hooks;
pub(crate) mod hooks_schema;
mod plugin;
pub(super) mod synthetic_plugin;

pub use error::{ApplyError, TomlError};
pub use plugin::HostFailure;
pub use synthetic_plugin::{
    PLUGIN_INSTALLATION_PREFERENCE, render_plugin_json, write_synthetic_plugin,
};

use crate::config::paths::{self, OrgPluginsLocation};
use crate::config::{self as config};
use crate::gateway::GatewayClient;
use crate::gateway::manifest::{ManagedMcpServer, SignedManifest, UserInfo};
use crate::sync::host_sync::{self, HostSyncCtx};
use std::fs;
use std::path::Path;
use systemprompt_identifiers::ValidatedUrl;
use url::{Host, Url};

pub(crate) use plugin::PluginApplyOutcome as ApplyReport;

pub(crate) async fn apply_manifest(
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

    let mut report = plugin::apply_plugins(client, bearer, manifest, root, &staging_root).await?;

    // Why: best-effort staging teardown; a leftover dir is reclaimed by the next
    // run's prepare_dirs and a removal failure here is not actionable.
    _ = fs::remove_dir_all(&staging_root);

    let mcp_servers = rewrite_loopback_urls(&manifest.managed_mcp_servers);
    let manifest_for_write = manifest_with_servers(manifest, mcp_servers.clone());
    write_user(&meta_dir, manifest.user.as_ref())?;

    crate::mcp_registry::publish(&mcp_servers);

    let ctx = HostSyncCtx {
        manifest: &manifest_for_write,
        org_plugins_root: root,
        client,
        bearer,
    };
    for emitter in host_sync::registry() {
        let host_id = emitter.host_id();
        let enabled = manifest_for_write
            .enabled_hosts
            .iter()
            .any(|h| h == host_id);
        let outcome = if enabled {
            emitter.apply(&ctx).await
        } else {
            emitter.clear()
        };
        if let Err(e) = &outcome {
            report.host_failures.push(HostFailure {
                host_id: host_id.to_string(),
                error: format!("{e:#}"),
            });
        }
        host_sync::log_outcome(host_id, enabled, outcome);
    }

    Ok(report)
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
    let (Some(raw_gw_host), gw_scheme) = (gateway_url.host_str(), gateway_url.scheme()) else {
        return servers.to_vec();
    };
    // Why: Cowork's MCP URL validator rejects the literal `localhost` for
    // non-HTTPS connectors — only `127.0.0.1` passes. Canonicalize once here so
    // both the explicit-loopback gateway case (operator configured
    // `http://localhost:48217`) and rewritten loopback MCP URLs end up
    // emitting `127.0.0.1`.
    let gw_host = if raw_gw_host.eq_ignore_ascii_case("localhost") {
        "127.0.0.1"
    } else {
        raw_gw_host
    };
    let gw_port = gateway_url.port();
    servers
        .iter()
        .map(|s| rewrite_loopback_server(s, gw_scheme, gw_host, gw_port))
        .collect()
}

fn rewrite_loopback_server(
    server: &ManagedMcpServer,
    gw_scheme: &str,
    gw_host: &str,
    gw_port: Option<u16>,
) -> ManagedMcpServer {
    let url_str = server.url.as_str();
    let Ok(mut parsed) = Url::parse(url_str) else {
        return server.clone();
    };
    let is_loopback = match parsed.host() {
        Some(Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(addr)) => addr.is_loopback(),
        Some(Host::Ipv6(addr)) => addr.is_loopback(),
        None => false,
    };
    if !is_loopback {
        return server.clone();
    }
    if parsed.set_scheme(gw_scheme).is_err() {
        return server.clone();
    }
    if parsed.set_host(Some(gw_host)).is_err() {
        return server.clone();
    }
    // Why: set_port returns Err for cannot-be-a-base URLs; for http(s) this only
    // fails on truly invalid input. Mirror gateway port (None clears the
    // explicit port).
    if parsed.set_port(gw_port).is_err() {
        return server.clone();
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
            let mut next = server.clone();
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
            server.clone()
        },
    }
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
    let meta_dir = paths::bridge_metadata_dir().ok_or_else(|| ApplyError::Io {
        context: "resolve bridge metadata dir".into(),
        source: std::io::Error::other("no LOCALAPPDATA / state dir resolvable"),
    })?;
    fs::create_dir_all(&meta_dir).map_err(|e| ApplyError::Io {
        context: format!("create metadata dir at {}", meta_dir.display()),
        source: e,
    })?;
    let staging_root = paths::bridge_staging_dir().ok_or_else(|| ApplyError::Io {
        context: "resolve bridge staging dir".into(),
        source: std::io::Error::other("no LOCALAPPDATA / state dir resolvable"),
    })?;
    // Why: clear any stale staging from an interrupted prior run; absence is the
    // normal case and a removal failure is recovered by the create_dir_all below.
    _ = fs::remove_dir_all(&staging_root);
    fs::create_dir_all(&staging_root).map_err(|e| ApplyError::Io {
        context: format!("create staging at {}", staging_root.display()),
        source: e,
    })?;
    Ok((meta_dir, staging_root))
}

// Why: Claude plugins ship the canonical `.claude-plugin/plugin.json`, but some
// trees use the dot-less `claude-plugin/`. Both the malformed-plugin check and
// the hooks-field injection must resolve whichever the synced tree actually
// uses, matching the dual-form lookup the GUI readers perform.
fn plugin_manifest_path(plugin_dir: &Path) -> Option<std::path::PathBuf> {
    use systemprompt_models::bridge::plugin_bundle::{PLUGIN_MANIFEST_DIRS, PLUGIN_MANIFEST_FILE};
    PLUGIN_MANIFEST_DIRS
        .iter()
        .map(|dir| plugin_dir.join(dir).join(PLUGIN_MANIFEST_FILE))
        .find(|path| path.is_file())
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
