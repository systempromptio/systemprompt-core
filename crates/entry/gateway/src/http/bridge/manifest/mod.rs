//! Bridge manifest endpoint.
//!
//! Loads auth, version, tenant, and per-user context, resolves the user's
//! catalogue through the `DynBridgeCatalogSource` the `managed_resources`
//! member provides (catalogue assembly, marketplace scoping and per-user
//! filtering), and signs the manifest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod per_user;
pub mod shape;

use std::time::Instant;

use axum::Json;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Duration, Utc};
use systemprompt_extension::bridge_catalog::{CatalogFreshness, ManifestEntries};
use systemprompt_gateway::surface::views::GatewayManifestSigner;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::{BridgePolicyConfig, BridgeReleasesSpec};
use systemprompt_models::bridge::instance::{BridgeVersionWindow, InstanceBranding};
use systemprompt_models::bridge::manifest::{
    MANIFEST_INLINE_SCHEMA_VERSION, SignedManifest, SignedManifestEnvelope,
    manifest_min_bridge_version,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;

use super::error::{BridgeError, authenticate_bridge};
use super::{BridgeRouteState, resolved};
use per_user::{PerUserContext, load_per_user_context, record_catalog_grants};
use systemprompt_extension::router::ApiHttpError;

pub async fn manifest(
    state: BridgeRouteState,
    headers: HeaderMap,
) -> Result<Response, ApiHttpError> {
    let started = Instant::now();
    let schema = shape::requested_schema(&headers);
    let (claims, _user) = authenticate_bridge(&state.authenticator, &headers).await?;
    let stamp = build_version()?;

    let services = state.host.services.view().config();
    let desktop_policy = desktop_policy(&services);
    let (branding, bridge_versions) = instance_presentation(&services);

    let (entries, bridge_policy) = assemble_candidate(
        &state,
        &claims.user_id,
        (*services).clone(),
        resolved::freshness_from_headers(&headers),
    )
    .await?;
    let resolved_at = Instant::now();
    let ManifestEntries {
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
    } = load_per_user_context(&state, &claims.user_id, services.instance_hosts()).await?;
    let host_extensions = services
        .instance_host_extensions()
        .into_iter()
        .filter(|(host, _)| enabled_hosts.contains(host))
        .collect();
    let per_user_at = Instant::now();

    let manifest = SignedManifest {
        min_schema_version: MANIFEST_INLINE_SCHEMA_VERSION,
        min_bridge_version: Some(manifest_min_bridge_version(&marketplaces)),
        manifest_version: stamp.manifest_version,
        issued_at: stamp.issued_at,
        not_before: stamp.not_before,
        user_id: claims.user_id,
        tenant_id: state
            .host
            .tenant_id
            .clone()
            .filter(|t| !t.as_str().is_empty()),
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
        host_extensions,
        artifacts,
        allow_claude_ai_connectors: bridge_policy.allow_claude_ai_connectors,
        desktop_policy,
        auto_update: bridge_policy.auto_update,
        allow_user_disable: bridge_policy.allow_user_disable,
        branding,
        bridge_versions,
        diagnostics,
        marketplaces,
    };

    Ok(respond(
        &state.signer,
        manifest,
        schema,
        &headers,
        [started, resolved_at, per_user_at],
    )?)
}

fn respond(
    signer: &GatewayManifestSigner,
    mut manifest: SignedManifest,
    schema: u32,
    headers: &HeaderMap,
    [started, resolved_at, per_user_at]: [Instant; 3],
) -> Result<Response, BridgeError> {
    shape::shape_for_schema(&mut manifest, schema);
    let etag = shape::entity_tag(&manifest)?;
    if shape::matches_if_none_match(headers, &etag) {
        log_timings(started, resolved_at, per_user_at, None);
        return Ok(with_validators(
            StatusCode::NOT_MODIFIED.into_response(),
            &etag,
        ));
    }

    let signing_at = Instant::now();
    let envelope: SignedManifestEnvelope = signer
        .seal(&manifest)
        .map_err(|e| BridgeError::internal("manifest signing failed", e))?;
    log_timings(started, resolved_at, per_user_at, Some(signing_at));
    Ok(with_validators(Json(envelope).into_response(), &etag))
}

// Why: `no-cache` is "revalidate before reuse" — the bridge keeps the signed
// envelope and sends its tag back, so an unchanged catalogue costs a 304.
fn with_validators(mut response: Response, etag: &str) -> Response {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    response
}

fn log_timings(
    started: Instant,
    resolved_at: Instant,
    per_user_at: Instant,
    signing_at: Option<Instant>,
) {
    let elapsed_ms = |from: Instant, to: Instant| to.saturating_duration_since(from).as_millis();
    let now = Instant::now();
    tracing::debug!(
        resolve_ms = elapsed_ms(started, resolved_at),
        per_user_ms = elapsed_ms(resolved_at, per_user_at),
        sign_ms = signing_at.map(|at| elapsed_ms(at, now)),
        elapsed_ms = elapsed_ms(started, now),
        not_modified = signing_at.is_none(),
        "bridge: manifest served"
    );
}

fn desktop_policy(
    services: &systemprompt_manifest::services::ServicesConfig,
) -> systemprompt_models::bridge::desktop_policy::DesktopPolicy {
    services
        .external_agents
        .values()
        .find(|agent| agent.id.as_str() == "claude_desktop")
        .map(|agent| agent.desktop_policy.clone())
        .unwrap_or_default()
}

fn instance_presentation(
    services: &systemprompt_manifest::services::ServicesConfig,
) -> (Option<InstanceBranding>, Option<BridgeVersionWindow>) {
    let gateway = services.gateway_config();
    let branding = gateway
        .and_then(|g| g.bridge_instance.as_ref())
        .map(|instance| instance.branding.to_branding())
        .filter(|branding| !branding.is_empty());
    let window = gateway
        .and_then(|g| g.bridge_releases.as_ref())
        .map(BridgeReleasesSpec::version_window)
        .filter(|window| !window.is_open());
    (branding, window)
}

pub async fn assemble_candidate(
    state: &BridgeRouteState,
    user_id: &UserId,
    services: systemprompt_manifest::services::ServicesConfig,
    freshness: CatalogFreshness,
) -> Result<(ManifestEntries, BridgePolicyConfig), BridgeError> {
    let bridge_policy = services.bridge_policy.unwrap_or_default();
    let resolved = resolved::resolve_for_user(state, &services, user_id, freshness)
        .await
        .map_err(|e| BridgeError::internal("manifest: catalogue resolution failed", e))?;
    record_catalog_grants(state, user_id, resolved.as_ref()).await?;
    Ok((resolved.manifest_entries(), bridge_policy))
}

struct ManifestStamp {
    manifest_version: ManifestVersion,
    issued_at: DateTime<Utc>,
    not_before: DateTime<Utc>,
}

fn build_version() -> Result<ManifestStamp, BridgeError> {
    let now = Utc::now();
    let issued_at = now;
    let not_before = now - Duration::seconds(60);
    let ts_millis = u64::try_from(now.timestamp_millis())
        .map_err(|e| BridgeError::internal("manifest version: timestamp overflow", e))?;
    let raw = format!("{}-{:016x}", now.format("%Y-%m-%dT%H:%M:%SZ"), ts_millis);
    let version = ManifestVersion::try_new(raw)
        .map_err(|e| BridgeError::internal("manifest version: invalid stamp", e))?;
    Ok(ManifestStamp {
        manifest_version: version,
        issued_at,
        not_before,
    })
}
