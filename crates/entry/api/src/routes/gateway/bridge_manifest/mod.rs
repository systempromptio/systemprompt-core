//! Bridge manifest endpoint.
//!
//! Loads auth, version, tenant, and per-user context, then delegates catalogue
//! assembly, marketplace scoping, per-user filtering, and signing to
//! `systemprompt_marketplace`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{Duration, Utc};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_marketplace::{ManifestService, MarketplaceCandidate};
use systemprompt_models::bridge::manifest::{
    MANIFEST_SCHEMA_VERSION, MIN_BRIDGE_VERSION, SignedManifest, SignedManifestEnvelope, UserInfo,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_runtime::AppContext;

use super::bridge::instance_enabled_hosts;
use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

pub async fn manifest(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<SignedManifestEnvelope>, (StatusCode, String)> {
    let claims = authenticate(&jwt_extractor, &headers).await?;
    let profile = profile_bootstrap()?;
    let tenant_id = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .filter(|t| !t.as_str().is_empty())
        .cloned();

    let (manifest_version, issued_at, not_before) = build_version()?;

    let services = bridge_data::load_services_config().map_err(|e| {
        tracing::warn!(error = %e, "manifest: services config load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("services: {e}"))
    })?;
    let instance_hosts = instance_enabled_hosts(&services);

    let (candidate, allow_claude_ai_connectors) =
        assemble_candidate(&ctx, profile, &claims.user_id, services).await?;
    let (entries, _filter_context) = candidate.into_manifest_parts();
    let systemprompt_marketplace::ManifestEntries {
        plugins,
        skills,
        agents,
        hooks,
        managed_mcp_servers,
        artifacts,
        diagnostics,
    } = entries;

    let PerUserContext {
        user,
        revocations,
        enabled_hosts,
        host_model_protocols,
    } = load_per_user_context(&ctx, &claims.user_id, instance_hosts).await;

    let manifest = SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: Some(MIN_BRIDGE_VERSION.to_owned()),
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
        host_model_protocols,
        artifacts,
        allow_claude_ai_connectors,
        diagnostics,
    };

    seal_manifest(&manifest).map(Json)
}

async fn assemble_candidate(
    ctx: &AppContext,
    profile: &systemprompt_models::Profile,
    user_id: &UserId,
    services: systemprompt_models::services::ServicesConfig,
) -> Result<(MarketplaceCandidate, bool), (StatusCode, String)> {
    let allow_claude_ai_connectors = services
        .bridge_policy
        .is_some_and(|p| p.allow_claude_ai_connectors);

    ManifestService::assemble_candidate(
        &services,
        ctx.app_paths().system().services(),
        &profile.server.api_external_url,
        ctx.marketplace_filter().as_ref(),
        user_id,
    )
    .await
    .map(|candidate| (candidate, allow_claude_ai_connectors))
    .map_err(|e| {
        tracing::warn!(error = %e, "manifest: candidate assembly failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("manifest: {e}"))
    })
}

struct PerUserContext {
    user: Option<UserInfo>,
    revocations: Vec<String>,
    enabled_hosts: Vec<String>,
    host_model_protocols: std::collections::BTreeMap<String, Vec<String>>,
}

async fn load_per_user_context(
    ctx: &AppContext,
    user_id: &UserId,
    instance_hosts: Vec<String>,
) -> PerUserContext {
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
        Ok(rows) if rows.is_empty() => instance_hosts,
        Ok(rows) => instance_hosts
            .into_iter()
            .filter(|h| rows.iter().any(|r| r == h))
            .collect(),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "manifest: enabled_hosts load failed; defaulting to instance-enabled hosts"
            );
            instance_hosts
        },
    };

    let host_model_protocols = match bridge_data::load_host_model_protocols(ctx, user_id).await {
        Ok(rows) => rows.into_iter().collect(),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "manifest: host model-protocol prefs load failed; continuing with defaults"
            );
            std::collections::BTreeMap::new()
        },
    };

    PerUserContext {
        user,
        revocations,
        enabled_hosts,
        host_model_protocols,
    }
}

fn seal_manifest(
    manifest: &SignedManifest,
) -> Result<SignedManifestEnvelope, (StatusCode, String)> {
    ManifestService::seal(manifest).map_err(|e| {
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
