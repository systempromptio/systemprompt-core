use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use systemprompt_agent::repository::content::{AgentRepository, SkillRepository};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{AgentName, JwtToken, TenantId, UserId};
use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ManagedMcpServer, PluginEntry, SignedManifest, SkillEntry, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_runtime::AppContext;
use systemprompt_security::manifest_signing;
use uuid::Uuid;

use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

#[derive(Debug, Deserialize)]
pub struct EnabledHostsRequest {
    pub host_id: String,
    pub enabled: bool,
}

pub async fn set_enabled_host(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Json(body): Json<EnabledHostsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_string(),
        )
    })?;
    let claims = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    if !KNOWN_HOSTS.iter().any(|h| *h == body.host_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unknown host: {}", body.host_id),
        ));
    }

    bridge_data::upsert_host_pref(&ctx, &claims.user_id, &body.host_id, body.enabled)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "host_id": body.host_id,
        "enabled": body.enabled,
    })))
}

pub async fn pubkey() -> impl IntoResponse {
    match manifest_signing::pubkey_b64() {
        Ok(b64) => (StatusCode::OK, Json(json!({ "pubkey": b64 }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize)]
pub struct BridgeProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

pub async fn profile() -> Result<Json<BridgeProfileResponse>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let gateway = profile
        .gateway
        .as_ref()
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_string()))?;

    let base = profile.server.api_external_url.trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let models: Vec<String> = gateway.catalog.as_ref().map_or_else(Vec::new, |catalog| {
        catalog.models.iter().map(|m| m.id.clone()).collect()
    });

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_deref())
        .map(canonicalize_org_uuid);

    Ok(Json(BridgeProfileResponse {
        inference_gateway_base_url,
        auth_scheme: gateway.auth_scheme.clone(),
        models,
        organization_uuid,
    }))
}

// Cowork rejects arbitrary strings (e.g. `local_198abcdef`) for its
// `deploymentOrganizationUuid` policy key — so even local-trial tenants need a
// valid v4/v5 UUID on the wire. Internal state keeps the `local_` prefix; only
// the Cowork-facing handler peels it.
fn canonicalize_org_uuid(tenant_id: &str) -> String {
    let suffix = tenant_id.strip_prefix("local_").unwrap_or(tenant_id);
    if let Ok(parsed) = Uuid::parse_str(suffix) {
        return parsed.to_string();
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, tenant_id.as_bytes()).to_string()
}

// JCS-canonical view used for signing. Must mirror the field set and order
// (alphabetical, after JCS sort) of the verifier-side `CanonicalView` in
// `bin/bridge/src/gateway/manifest.rs` so signer + verifier produce identical
// canonical bytes.
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

const KNOWN_HOSTS: &[&str] = &["claude-code", "claude-desktop", "cowork", "codex-cli"];

fn default_enabled_hosts() -> Vec<String> {
    KNOWN_HOSTS.iter().map(|s| (*s).to_string()).collect()
}

pub async fn manifest(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<SignedManifest>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_string(),
        )
    })?;
    let claims = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_deref())
        .filter(|t| !t.is_empty())
        .map(TenantId::new);

    let now = Utc::now();
    let issued_at = now.to_rfc3339();
    let not_before = (now - Duration::seconds(60)).to_rfc3339();
    let ts_millis = u64::try_from(now.timestamp_millis()).unwrap_or(0);
    let manifest_version_raw = format!("{}-{:016x}", now.format("%Y-%m-%dT%H:%M:%SZ"), ts_millis);
    let manifest_version = ManifestVersion::try_new(manifest_version_raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("manifest version: {e}"),
        )
    })?;

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
