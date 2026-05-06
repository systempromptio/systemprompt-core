use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::{Duration, Utc};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use systemprompt_agent::repository::content::{AgentRepository, SkillRepository};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{AgentName, JwtToken, TenantId, UserId};
use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, SkillId, SkillName};
use systemprompt_models::bridge::manifest::{
    AgentEntry, ManagedMcpServer, PluginEntry, SignedManifest, SkillEntry,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_runtime::AppContext;
use systemprompt_security::manifest_signing;
use uuid::Uuid;

use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

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
    user: Option<&'a systemprompt_models::bridge::manifest::UserInfo>,
    plugins: &'a [PluginEntry],
    skills: &'a [SkillEntry],
    agents: &'a [AgentEntry],
    managed_mcp_servers: &'a [ManagedMcpServer],
    revocations: &'a [String],
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
    let manifest_version_raw = format!(
        "{}-{:016x}",
        now.format("%Y-%m-%dT%H:%M:%SZ"),
        ts_millis
    );
    let manifest_version = ManifestVersion::try_new(manifest_version_raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("manifest version: {e}"),
        )
    })?;

    let skills = load_skills(&ctx).await.map_err(|e| {
        tracing::warn!(error = %e, "manifest: skill load failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("skills: {e}"),
        )
    })?;

    let agents = load_agents(&ctx, &profile.server.api_external_url)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "manifest: agent load failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("agents: {e}"),
            )
        })?;

    // Plugins, managed MCP servers, and revocations are not yet sourced from
    // server-side state: plugin sha256/files are computed by the bridge at
    // upload time, the services-config `Deployment` shape carries no public
    // URL, and revocations have no DB representation. Emitting empty arrays
    // keeps the wire contract honest until those data paths land.
    let plugins: Vec<PluginEntry> = Vec::new();
    let managed_mcp_servers: Vec<ManagedMcpServer> = Vec::new();
    let revocations: Vec<String> = Vec::new();

    let canonical = CanonicalView {
        manifest_version: &manifest_version,
        issued_at: &issued_at,
        not_before: &not_before,
        user_id: &claims.user_id,
        tenant_id: tenant_id.as_ref(),
        user: None,
        plugins: &plugins,
        skills: &skills,
        agents: &agents,
        managed_mcp_servers: &managed_mcp_servers,
        revocations: &revocations,
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
        user: None,
        plugins,
        skills,
        agents,
        managed_mcp_servers,
        revocations,
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

async fn load_agents(
    ctx: &AppContext,
    api_external_url: &str,
) -> anyhow::Result<Vec<AgentEntry>> {
    let repo = AgentRepository::new(ctx.db_pool())?;
    let rows = repo.list_enabled().await?;
    let base = api_external_url.trim_end_matches('/');
    let mut out = Vec::with_capacity(rows.len());
    for agent in rows {
        let name = AgentName::try_new(agent.name.clone())?;
        let endpoint = if agent.endpoint.starts_with("http://")
            || agent.endpoint.starts_with("https://")
        {
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
