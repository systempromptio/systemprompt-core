use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use systemprompt_identifiers::{UserId, ValidatedUrl};
use systemprompt_loader::ConfigLoader;
use systemprompt_models::bridge::ids::{ManagedMcpServerName, PluginId, Sha256Digest};
use systemprompt_models::bridge::manifest::{ManagedMcpServer, PluginEntry, PluginFile, UserInfo};
use systemprompt_models::services::{PluginConfig, ServicesConfig};
use systemprompt_oauth::repository::BridgeHostPrefsRepository;
use systemprompt_runtime::AppContext;
use systemprompt_users::UserRepository;

const PLUGIN_BLOCKED_FILENAMES: &[&str] = &["config.yaml", "config.yml"];

pub async fn load_user(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Option<UserInfo>> {
    let repo = UserRepository::new(ctx.db_pool())?;
    let Some(user) = repo.find_by_id(user_id).await? else {
        return Ok(None);
    };
    Ok(Some(UserInfo {
        id: user.id,
        name: user.name,
        email: user.email,
        display_name: user.display_name,
        roles: user.roles,
    }))
}

pub async fn load_revocations(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Vec<String>> {
    let repo = UserRepository::new(ctx.db_pool())?;
    let ids = repo.list_revoked_api_key_ids_for_user(user_id).await?;
    Ok(ids)
}

pub async fn load_enabled_hosts(ctx: &AppContext, user_id: &UserId) -> anyhow::Result<Vec<String>> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    Ok(repo.list_enabled(user_id).await?)
}

pub async fn upsert_host_pref(
    ctx: &AppContext,
    user_id: &UserId,
    host_id: &str,
    enabled: bool,
) -> anyhow::Result<()> {
    let repo = BridgeHostPrefsRepository::new(ctx.db_pool())?;
    repo.upsert(user_id, host_id, enabled).await?;
    Ok(())
}

pub fn load_services_config() -> anyhow::Result<ServicesConfig> {
    ConfigLoader::load().map_err(|e| anyhow::anyhow!("services config load: {e}"))
}

pub fn load_managed_mcp_servers(
    services: &ServicesConfig,
    api_external_url: &str,
) -> anyhow::Result<Vec<ManagedMcpServer>> {
    let base = api_external_url.trim_end_matches('/');
    let mut entries: Vec<(&String, &systemprompt_models::mcp::Deployment)> = services
        .mcp_servers
        .iter()
        .filter(|(_, d)| d.enabled)
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (name, deployment) in entries {
        let url_str = if deployment.endpoint.starts_with("http://")
            || deployment.endpoint.starts_with("https://")
        {
            deployment.endpoint.clone()
        } else {
            format!("{base}/api/v1/mcp/{name}/mcp")
        };
        let url = ValidatedUrl::try_new(url_str)?;
        let mcp_name = ManagedMcpServerName::try_new(name.clone())?;
        out.push(ManagedMcpServer {
            name: mcp_name,
            url,
            transport: Some("http".to_owned()),
            headers: None,
            oauth: Some(deployment.oauth.required),
            tool_policy: None,
        });
    }
    Ok(out)
}

pub fn load_plugins(ctx: &AppContext, services: &ServicesConfig) -> Vec<PluginEntry> {
    let plugins_root: PathBuf = ctx.app_paths().system().services().join("plugins");
    let mut configs: Vec<&PluginConfig> = services.plugins.values().filter(|p| p.enabled).collect();
    configs.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut out = Vec::with_capacity(configs.len());
    for config in configs {
        match build_plugin_entry(&plugins_root, config) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    "manifest: plugin directory missing on disk; skipping"
                );
            },
            Err(e) => {
                tracing::warn!(
                    plugin_id = %config.id,
                    error = %e,
                    "manifest: failed to build plugin entry; skipping"
                );
            },
        }
    }
    out
}

fn build_plugin_entry(
    plugins_root: &Path,
    config: &PluginConfig,
) -> anyhow::Result<Option<PluginEntry>> {
    let plugin_dir = plugins_root.join(config.id.as_str());
    if !plugin_dir.is_dir() {
        return Ok(None);
    }

    let mut files: BTreeMap<String, PluginFile> = BTreeMap::new();
    collect_files(&plugin_dir, &plugin_dir, &mut files)?;
    let mut hasher = Sha256::new();
    hasher.update(config.id.as_str().as_bytes());
    hasher.update(config.version.as_bytes());
    for file in files.values() {
        hasher.update(file.path.as_bytes());
        hasher.update(file.sha256.as_str().as_bytes());
    }
    let aggregate = hex::encode(hasher.finalize());
    let sha256 = Sha256Digest::try_new(aggregate)?;
    let id = PluginId::try_new(config.id.as_str())?;
    Ok(Some(PluginEntry {
        id,
        version: config.version.clone(),
        sha256,
        files: files.into_values().collect(),
    }))
}

pub(super) fn collect_files(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, PluginFile>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|f| f.to_str()) {
            if PLUGIN_BLOCKED_FILENAMES.contains(&name) {
                continue;
            }
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let Some(rel_str) = rel.to_str() else {
            continue;
        };
        let normalized = rel_str.replace('\\', "/");
        let bytes = std::fs::read(&path)?;
        let size = bytes.len() as u64;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let digest = hex::encode(hasher.finalize());
        let sha256 = Sha256Digest::try_new(digest)?;
        out.insert(
            normalized.clone(),
            PluginFile {
                path: normalized,
                sha256,
                size,
            },
        );
    }
    Ok(())
}
