//! Bridge manifest endpoint.
//!
//! Loads auth, version, tenant, and per-user context, then delegates catalogue
//! assembly, marketplace scoping, per-user filtering, and signing to
//! `systemprompt_marketplace`.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_marketplace::{CanonicalView, ManifestService, MarketplaceCandidate};
use systemprompt_models::bridge::ids::ManifestSignature;
use systemprompt_models::bridge::manifest::{SignedManifest, UserInfo};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_runtime::AppContext;

use super::bridge::KNOWN_HOSTS;
use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

fn default_enabled_hosts() -> Vec<String> {
    KNOWN_HOSTS.iter().map(|s| (*s).to_owned()).collect()
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
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .filter(|t| !t.as_str().is_empty())
        .cloned();

    let (manifest_version, issued_at, not_before) = build_version()?;

    let MarketplaceCandidate {
        plugins,
        skills,
        agents,
        hooks,
        managed_mcp_servers,
        // Why: marketplace_id/access are filter context, deliberately excluded
        // from CanonicalView so the signed payload stays byte-identical.
        ..
    } = assemble_candidate(&ctx, profile, &claims.user_id).await?;

    let PerUserContext {
        user,
        revocations,
        enabled_hosts,
    } = load_per_user_context(&ctx, &claims.user_id).await;

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

    let signature = sign_canonical(&canonical)?;

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
        signature,
    }))
}

async fn assemble_candidate(
    ctx: &AppContext,
    profile: &systemprompt_models::Profile,
    user_id: &UserId,
) -> Result<MarketplaceCandidate, (StatusCode, String)> {
    let services = bridge_data::load_services_config().map_err(|e| {
        tracing::warn!(error = %e, "manifest: services config load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("services: {e}"))
    })?;

    ManifestService::assemble_candidate(
        &services,
        ctx.app_paths().system().services(),
        &profile.server.api_external_url,
        ctx.marketplace_filter().as_ref(),
        user_id,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = %e, "manifest: candidate assembly failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("manifest: {e}"))
    })
}

struct PerUserContext {
    user: Option<UserInfo>,
    revocations: Vec<String>,
    enabled_hosts: Vec<String>,
}

async fn load_per_user_context(ctx: &AppContext, user_id: &UserId) -> PerUserContext {
    let user = match bridge_data::load_user(ctx, user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: user load failed; continuing without user");
            None
        },
    };

    let revocations = match bridge_data::load_revocations(ctx, user_id).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: revocation load failed; continuing empty");
            Vec::new()
        },
    };

    let enabled_hosts = match bridge_data::load_enabled_hosts(ctx, user_id).await {
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

    PerUserContext {
        user,
        revocations,
        enabled_hosts,
    }
}

fn sign_canonical(
    canonical: &CanonicalView<'_>,
) -> Result<ManifestSignature, (StatusCode, String)> {
    ManifestService::sign(canonical).map_err(|e| {
        tracing::error!(error = %e, "manifest signing failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("manifest signing failed: {e}"),
        )
    })
}

async fn authenticate(
    jwt_extractor: &JwtContextExtractor,
    headers: &HeaderMap,
) -> Result<crate::services::middleware::jwt::JwtUserContext, (StatusCode, String)> {
    let credential = extract_credential(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map(|(claims, _user)| claims)
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
    let ts_millis = u64::try_from(now.timestamp_millis()).map_err(|_e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "manifest version: timestamp overflow".to_owned(),
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
