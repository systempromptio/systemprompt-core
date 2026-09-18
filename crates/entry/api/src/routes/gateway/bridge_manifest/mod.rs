//! Bridge manifest endpoint.
//!
//! Loads auth, version, tenant, and per-user context, then delegates catalogue
//! assembly, marketplace scoping, per-user filtering, and signing to
//! `systemprompt_marketplace`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod per_user;

use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use chrono::{DateTime, Duration, Utc};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{JwtToken, UserId};
use systemprompt_marketplace::{ManifestService, MarketplaceCandidate};
use systemprompt_models::bridge::manifest::{
    MANIFEST_SCHEMA_VERSION, SignedManifest, SignedManifestEnvelope, min_bridge_version,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;
use systemprompt_models::services::BridgePolicyConfig;
use systemprompt_runtime::AppContext;

use super::bridge::instance_enabled_hosts;
use super::messages::extract_credential;
use super::{bridge_data, bridge_resolved};
use crate::services::middleware::JwtContextExtractor;
use per_user::{PerUserContext, load_per_user_context, record_catalog_grants};

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

    let ManifestStamp {
        manifest_version,
        issued_at,
        not_before,
    } = build_version()?;

    let services = bridge_data::load_services_config().map_err(|e| {
        tracing::warn!(error = %e, "manifest: services config load failed");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("services: {e}"))
    })?;
    let instance_hosts = instance_enabled_hosts(&services);

    let (candidate, bridge_policy) =
        assemble_candidate(&ctx, profile, &claims.user_id, services).await?;
    let (entries, _filter_context) = candidate.into_manifest_parts();
    let systemprompt_marketplace::ManifestEntries {
        plugins,
        skills,
        rules,
        agents,
        hooks,
        managed_mcp_servers,
        artifacts,
        marketplaces,
        diagnostics,
    } = entries;

    let PerUserContext {
        user,
        revocations,
        enabled_hosts,
        host_model_protocols,
    } = load_per_user_context(&ctx, &claims.user_id, instance_hosts).await?;

    let manifest = SignedManifest {
        min_schema_version: MANIFEST_SCHEMA_VERSION,
        min_bridge_version: Some(min_bridge_version()),
        manifest_version,
        issued_at,
        not_before,
        user_id: claims.user_id,
        tenant_id,
        user,
        plugins,
        skills,
        rules,
        agents,
        hooks,
        managed_mcp_servers,
        revocations,
        enabled_hosts,
        host_model_protocols,
        artifacts,
        allow_claude_ai_connectors: bridge_policy.allow_claude_ai_connectors,
        auto_update: bridge_policy.auto_update,
        diagnostics,
        marketplaces,
    };

    seal_manifest(&manifest).map(Json)
}

pub(crate) async fn assemble_candidate(
    ctx: &AppContext,
    profile: &systemprompt_models::Profile,
    user_id: &UserId,
    services: systemprompt_models::services::ServicesConfig,
) -> Result<(MarketplaceCandidate, BridgePolicyConfig), (StatusCode, String)> {
    let bridge_policy = services.bridge_policy.unwrap_or_default();
    let resolved = bridge_resolved::resolve_for_user(ctx, &services, profile, user_id)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "manifest: catalogue resolution failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("manifest: {error}"),
            )
        })?;
    record_catalog_grants(ctx, user_id, &resolved.candidate).await?;
    Ok(((*resolved.candidate).clone(), bridge_policy))
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

struct ManifestStamp {
    manifest_version: ManifestVersion,
    issued_at: DateTime<Utc>,
    not_before: DateTime<Utc>,
}

fn build_version() -> Result<ManifestStamp, (StatusCode, String)> {
    let now = Utc::now();
    let issued_at = now;
    let not_before = now - Duration::seconds(60);
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
    Ok(ManifestStamp {
        manifest_version: version,
        issued_at,
        not_before,
    })
}
