use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use systemprompt_agent::repository::content::{AgentRepository, SkillRepository};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{AgentName, JwtToken, TenantId, UserId};
use systemprompt_marketplace::MarketplaceCandidate;
use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ManagedMcpServer, PluginEntry, SignedManifest, SkillEntry, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
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

    let skills = load_skills(&ctx).await.map_err(|e| {
        tracing::warn!(error = %e, "manifest: skill load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("skills: {e}"))
    })?;

    let agents = load_agents(&ctx, &profile.server.api_external_url)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "manifest: agent load failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("agents: {e}"))
        })?;

    let services = bridge_data::load_services_config().map_err(|e| {
        tracing::warn!(error = %e, "manifest: services config load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("services: {e}"))
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
            MarketplaceCandidate::new(plugins, skills, agents, managed_mcp_servers),
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

async fn load_skills(ctx: &AppContext) -> anyhow::Result<Vec<SkillEntry>> {
    let repo = SkillRepository::new(ctx.db_pool())?;
    let rows = repo.list_enabled().await?;
    let mut out = Vec::with_capacity(rows.len());
    for skill in rows {
        let mut hasher = Sha256::new();
        hasher.update(skill.instructions.as_bytes());
        let digest = hex::encode(hasher.finalize());
        let sha256 = Sha256Digest::try_new(digest)?;
        let id = SkillId::try_new(skill.id.as_str())?;
        let name = SkillName::try_new(skill.name.clone())?;
        out.push(SkillEntry {
            id,
            name,
            description: skill.description,
            file_path: skill.file_path,
            tags: skill.tags,
            sha256,
            instructions: skill.instructions,
        });
    }
    Ok(out)
}

async fn load_agents(ctx: &AppContext, api_external_url: &str) -> anyhow::Result<Vec<AgentEntry>> {
    let repo = AgentRepository::new(ctx.db_pool())?;
    let rows = repo.list_enabled().await?;
    let base = api_external_url.trim_end_matches('/');
    let mut out = Vec::with_capacity(rows.len());
    for agent in rows {
        let name = AgentName::try_new(agent.name.clone())?;
        let endpoint =
            if agent.endpoint.starts_with("http://") || agent.endpoint.starts_with("https://") {
                agent.endpoint.clone()
            } else {
                format!("{base}{}", agent.endpoint)
            };
        out.push(AgentEntry {
            id: agent.id,
            name,
            display_name: agent.display_name,
            description: agent.description,
            version: agent.version,
            endpoint,
            enabled: agent.enabled,
            is_default: agent.is_default,
            is_primary: agent.is_primary,
            provider: agent.provider,
            model: agent.model,
            mcp_servers: agent.mcp_servers,
            skills: agent.skills,
            tags: agent.tags,
            system_prompt: agent.system_prompt,
        });
    }
    Ok(out)
}
