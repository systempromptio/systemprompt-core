//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode, header};
use systemprompt_identifiers::ManagedResourceId;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::feedback::receipts::AuthenticatedConsumerDevice;
use systemprompt_runtime::AppContext;

use super::error::ConsumerHttpError;

pub(super) fn credential(headers: &HeaderMap) -> Result<&str, ConsumerHttpError> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.starts_with("sp_device_") && value.len() <= 256)
        .ok_or(ConsumerHttpError(StatusCode::UNAUTHORIZED))
}

// Why: an unknown or revoked credential is the caller's 401; a storage
// failure while checking it is not and keeps its 5xx classification.
pub(super) async fn authenticate_device(
    ctx: &AppContext,
    credential: &str,
) -> Result<AuthenticatedConsumerDevice, ConsumerHttpError> {
    match ctx
        .managed_repository()
        .authenticate_consumer_device(credential)
        .await
    {
        Ok(identity) => Ok(identity),
        Err(ManagedError::Unavailable) => Err(ConsumerHttpError(StatusCode::UNAUTHORIZED)),
        Err(error) => Err(ConsumerHttpError::from(error)),
    }
}

pub(super) async fn resource(
    ctx: &AppContext,
    credential: &str,
    resource: &ManagedResourceId,
    host: EvaluatorClient,
) -> Result<(), ConsumerHttpError> {
    let identity = authenticate_device(ctx, credential).await?;
    let profile = systemprompt_config::ProfileBootstrap::get().map_err(|error| {
        tracing::warn!(%error, "consumer: profile unavailable");
        ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE)
    })?;
    let services =
        crate::routes::gateway::bridge_data::load_services_config().map_err(|error| {
            tracing::warn!(%error, "consumer: services config load failed");
            ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE)
        })?;
    if !crate::routes::gateway::bridge::instance_enabled_hosts(&services)
        .iter()
        .any(|value| host.accepts_host_name(value))
    {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    let (candidate, _) = crate::routes::gateway::bridge_manifest::assemble_candidate(
        ctx,
        profile,
        &identity.consumer_id,
        services,
        crate::routes::gateway::bridge_resolved::Freshness::Memo,
    )
    .await
    .map_err(|(status, detail)| {
        tracing::warn!(%status, detail, "consumer: candidate assembly failed");
        ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE)
    })?;
    let (entries, _) = candidate.into_manifest_parts();
    if !entries.skills.iter().any(|skill| {
        skill
            .publication
            .as_ref()
            .is_some_and(|publication| &publication.resource_id == resource)
            && (skill.hosts.is_empty()
                || skill
                    .hosts
                    .iter()
                    .any(|value| host.accepts_host_name(value)))
    }) {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    let owner = ctx
        .managed_repository()
        .consumer_resource_owner(resource)
        .await?;
    ctx.managed_repository()
        .retain_consumer_catalog_grant(&owner, resource, &identity.consumer_id)
        .await?;
    Ok(())
}
