use std::path::Path;
use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{AgentId, AgentName, HookId, JwtToken, TenantId, UserId};
use systemprompt_marketplace::MarketplaceCandidate;
use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{
    AgentEntry, HookEntry, ManagedMcpServer, PluginEntry, SignedManifest, SkillEntry, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_models::services::hooks::HOOK_CONFIG_FILENAME;
use systemprompt_models::services::{
    AgentConfig, DiskHookConfig, DiskSkillConfig, SKILL_CONFIG_FILENAME, ServicesConfig,
    strip_frontmatter,
};
use systemprompt_runtime::AppContext;
use systemprompt_security::manifest_signing;

use super::bridge::KNOWN_HOSTS;
use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

// Why: must mirror the field set and order (alphabetical, after JCS sort) of
// the verifier-side `CanonicalView` in `bin/bridge/src/gateway/manifest.rs` so
// signer + verifier produce identical canonical bytes.
#[derive(Serialize)]
struct CanonicalView<'a> {
    manifest_version: &'a ManifestVersion,
    issued_at: &'a str,
    not_before: &'a str,
    user_id: &'a UserId,
    tenant_id: Option<&'a TenantId>,
    user: Option<&'a UserInfo>,
    plugins: &'a [PluginEntry],
    skills: &'a [SkillEntry],
    agents: &'a [AgentEntry],
    hooks: &'a [HookEntry],
    managed_mcp_servers: &'a [ManagedMcpServer],
    revocations: &'a [String],
    enabled_hosts: &'a [String],
}

fn default_enabled_hosts() -> Vec<String> {
    KNOWN_HOSTS.iter().map(|s| (*s).to_string()).collect()
}

pub async fn manifest(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<SignedManifest>, (StatusCode, String)> {
    let claims = authenticate(&jwt_extractor, &headers).await?;
    let profile = profile_bootstrap()?;
    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_deref())
        .filter(|t| !t.is_empty())
        .map(TenantId::new);

    let (manifest_version, issued_at, not_before) = build_version()?;

    let services = bridge_data::load_services_config().map_err(|e| {
        tracing::warn!(error = %e, "manifest: services config load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("services: {e}"))
    })?;

    let services_root = ctx.app_paths().system().services();

    let skills = load_skills(services_root).map_err(|e| {
        tracing::warn!(error = %e, "manifest: skill load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("skills: {e}"))
    })?;

    let agents = load_agents(&services, &profile.server.api_external_url);

    let hooks = load_hooks(services_root).map_err(|e| {
        tracing::warn!(error = %e, "manifest: hook load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("hooks: {e}"))
    })?;

    let plugins = bridge_data::load_plugins(&ctx, &services);

    let managed_mcp_servers =
        bridge_data::load_managed_mcp_servers(&services, &profile.server.api_external_url)
            .map_err(|e| {
                tracing::warn!(error = %e, "manifest: managed mcp load failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("managed mcp: {e}"),
                )
            })?;

    let user = match bridge_data::load_user(&ctx, &claims.user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: user load failed; continuing without user");
            None
        },
    };

    let revocations = match bridge_data::load_revocations(&ctx, &claims.user_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: revocation load failed; continuing empty");
            Vec::new()
        },
    };

    let enabled_hosts = match bridge_data::load_enabled_hosts(&ctx, &claims.user_id).await {
        Ok(rows) if rows.is_empty() => default_enabled_hosts(),
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "manifest: enabled_hosts load failed; defaulting to all known hosts"
            );
            default_enabled_hosts()
        },
    };

    let filtered = ctx
        .marketplace_filter()
        .filter(
            &claims.user_id,
            MarketplaceCandidate::new(plugins, skills, agents, hooks, managed_mcp_servers),
        )
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "manifest: marketplace filter rejected request");
            (StatusCode::FORBIDDEN, format!("marketplace filter: {e}"))
        })?;
    let MarketplaceCandidate {
        plugins,
        skills,
        agents,
        hooks,
        managed_mcp_servers,
    } = filtered;

    let canonical = CanonicalView {
        manifest_version: &manifest_version,
        issued_at: &issued_at,
        not_before: &not_before,
        user_id: &claims.user_id,
        tenant_id: tenant_id.as_ref(),
        user: user.as_ref(),
        plugins: &plugins,
        skills: &skills,
        agents: &agents,
        hooks: &hooks,
        managed_mcp_servers: &managed_mcp_servers,
        revocations: &revocations,
        enabled_hosts: &enabled_hosts,
    };

    let signature = manifest_signing::sign_value(&canonical).map_err(|e| {
        tracing::error!(error = %e, "manifest signing failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("manifest signing failed: {e}"),
        )
    })?;

    Ok(Json(SignedManifest {
        manifest_version,
        issued_at,
        not_before,
        user_id: claims.user_id,
        tenant_id,
        user,
        plugins,
        skills,
        agents,
        hooks,
        managed_mcp_servers,
        revocations,
        enabled_hosts,
        signature: ManifestSignature::new(signature),
    }))
}

async fn authenticate(
    jwt_extractor: &JwtContextExtractor,
    headers: &HeaderMap,
) -> Result<crate::services::middleware::jwt::JwtUserContext, (StatusCode, String)> {
    let credential = extract_credential(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_string(),
        )
    })?;
    jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))
}

fn profile_bootstrap() -> Result<&'static systemprompt_models::Profile, (StatusCode, String)> {
    ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })
}

fn build_version() -> Result<(ManifestVersion, String, String), (StatusCode, String)> {
    let now = Utc::now();
    let issued_at = now.to_rfc3339();
    let not_before = (now - Duration::seconds(60)).to_rfc3339();
    let ts_millis = u64::try_from(now.timestamp_millis()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "manifest version: timestamp overflow".to_string(),
        )
    })?;
    let raw = format!("{}-{:016x}", now.format("%Y-%m-%dT%H:%M:%SZ"), ts_millis);
    let version = ManifestVersion::try_new(raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("manifest version: {e}"),
        )
    })?;
    Ok((version, issued_at, not_before))
}

fn load_skills(services_root: &Path) -> anyhow::Result<Vec<SkillEntry>> {
    let skills_dir = services_root.join("skills");
    if !skills_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&skills_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let config_path = path.join(SKILL_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }
        entries.push((dir_name.to_string(), path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (dir_name, skill_dir) in entries {
        match build_skill_entry(&dir_name, &skill_dir) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {},
            Err(e) => {
                tracing::warn!(
                    skill_dir = %skill_dir.display(),
                    error = %e,
                    "manifest: failed to build skill entry; skipping"
                );
            },
        }
    }
    Ok(out)
}

fn build_skill_entry(dir_name: &str, skill_dir: &Path) -> anyhow::Result<Option<SkillEntry>> {
    let config_path = skill_dir.join(SKILL_CONFIG_FILENAME);
    let config_text = std::fs::read_to_string(&config_path)?;
    let config: DiskSkillConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", config_path.display()))?;

    if !config.enabled {
        return Ok(None);
    }

    let id = if config.id.as_str().is_empty() {
        SkillId::try_new(dir_name.replace('-', "_"))?
    } else {
        SkillId::try_new(config.id.as_str())?
    };
    let display_name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name.clone()
    };
    let name = SkillName::try_new(display_name)?;

    let content_path = skill_dir.join(config.content_file());
    let instructions = if content_path.exists() {
        let raw = std::fs::read_to_string(&content_path)?;
        strip_frontmatter(&raw)
    } else {
        String::new()
    };

    let mut hasher = Sha256::new();
    hasher.update(instructions.as_bytes());
    let sha256 = Sha256Digest::try_new(hex::encode(hasher.finalize()))?;

    Ok(Some(SkillEntry {
        id,
        name,
        description: config.description,
        file_path: content_path.to_string_lossy().into_owned(),
        tags: config.tags,
        sha256,
        instructions,
    }))
}

fn load_agents(services: &ServicesConfig, api_external_url: &str) -> Vec<AgentEntry> {
    let base = api_external_url.trim_end_matches('/');
    let mut keys: Vec<&String> = services
        .agents
        .iter()
        .filter(|(_, cfg)| cfg.enabled)
        .map(|(k, _)| k)
        .collect();
    keys.sort();

    let mut out = Vec::with_capacity(keys.len());
    for key in keys {
        let cfg = &services.agents[key];
        match build_agent_entry(key, cfg, base) {
            Ok(entry) => out.push(entry),
            Err(e) => {
                tracing::warn!(
                    agent = %key,
                    error = %e,
                    "manifest: failed to build agent entry; skipping"
                );
            },
        }
    }
    out
}

fn build_agent_entry(key: &str, cfg: &AgentConfig, base: &str) -> anyhow::Result<AgentEntry> {
    let id = AgentId::new(key);
    let name = AgentName::try_new(cfg.name.clone())?;
    let endpoint = if cfg.endpoint.starts_with("http://") || cfg.endpoint.starts_with("https://") {
        cfg.endpoint.clone()
    } else if cfg.endpoint.is_empty() {
        format!("{base}/api/v1/agents/{}", cfg.name)
    } else {
        format!("{base}{}", cfg.endpoint)
    };

    let display_name = cfg.card.display_name.clone();
    let description = cfg.card.description.clone();
    let version = cfg.card.version.clone();

    Ok(AgentEntry {
        id,
        name,
        display_name,
        description,
        version,
        endpoint,
        enabled: cfg.enabled,
        is_default: cfg.default,
        is_primary: cfg.is_primary,
        provider: cfg.metadata.provider.clone(),
        model: cfg.metadata.model.clone(),
        mcp_servers: cfg.metadata.mcp_servers.clone(),
        skills: cfg.metadata.skills.clone(),
        tags: cfg.tags.clone(),
        system_prompt: cfg.metadata.system_prompt.clone(),
    })
}

fn load_hooks(services_root: &Path) -> anyhow::Result<Vec<HookEntry>> {
    let hooks_dir = services_root.join("hooks");
    if !hooks_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&hooks_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let config_path = path.join(HOOK_CONFIG_FILENAME);
        if !config_path.exists() {
            continue;
        }
        entries.push((dir_name.to_string(), config_path));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = Vec::with_capacity(entries.len());
    for (dir_name, config_path) in entries {
        match build_hook_entry(&dir_name, &config_path) {
            Ok(Some(entry)) => out.push(entry),
            Ok(None) => {},
            Err(e) => {
                tracing::warn!(
                    hook_dir = %dir_name,
                    error = %e,
                    "manifest: failed to build hook entry; skipping"
                );
            },
        }
    }
    Ok(out)
}

fn build_hook_entry(dir_name: &str, config_path: &Path) -> anyhow::Result<Option<HookEntry>> {
    let config_text = std::fs::read_to_string(config_path)?;
    let config: DiskHookConfig = serde_yaml::from_str(&config_text)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", config_path.display()))?;

    if !config.enabled {
        return Ok(None);
    }

    let id = if config.id.as_str().is_empty() {
        HookId::new(dir_name.replace('-', "_"))
    } else {
        HookId::new(config.id.as_str())
    };
    let name = if config.name.is_empty() {
        dir_name.replace('_', " ")
    } else {
        config.name.clone()
    };

    let mut hasher = Sha256::new();
    hasher.update(config_text.as_bytes());
    let sha256 = Sha256Digest::try_new(hex::encode(hasher.finalize()))?;

    Ok(Some(HookEntry {
        id,
        name,
        description: config.description,
        version: config.version,
        event: config.event,
        matcher: config.matcher,
        command: config.command,
        is_async: config.is_async,
        category: config.category,
        tags: config.tags,
        sha256,
    }))
}
