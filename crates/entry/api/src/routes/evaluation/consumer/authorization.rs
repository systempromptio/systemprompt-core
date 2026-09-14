//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::{HeaderMap, StatusCode, header};
use systemprompt_identifiers::ManagedResourceId;
use systemprompt_models::feedback::EvaluatorClient;
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

pub(super) async fn resource(
    ctx: &AppContext,
    credential: &str,
    resource: &ManagedResourceId,
    host: EvaluatorClient,
) -> Result<(), ConsumerHttpError> {
    let identity = ctx
        .managed_repository()
        .authenticate_consumer_device(credential)
        .await
        .map_err(|_| ConsumerHttpError(StatusCode::UNAUTHORIZED))?;
    let profile = systemprompt_config::ProfileBootstrap::get()
        .map_err(|_| ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE))?;
    let services = crate::routes::gateway::bridge_data::load_services_config()
        .map_err(|_| ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE))?;
    let host = systemprompt_marketplace::managed::consumer::host_key(host);
    if !crate::routes::gateway::bridge::instance_enabled_hosts(&services)
        .iter()
        .any(|value| value == host)
    {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    let (candidate, _) = crate::routes::gateway::bridge_manifest::assemble_candidate(
        ctx,
        profile,
        &identity.consumer_id,
        services,
    )
    .await
    .map_err(|_| ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE))?;
    let (entries, _) = candidate.into_manifest_parts();
    if !entries.skills.iter().any(|skill| {
        skill
            .publication
            .as_ref()
            .is_some_and(|publication| &publication.resource_id == resource)
            && (skill.hosts.is_empty() || skill.hosts.iter().any(|value| value == host))
    }) {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    ctx.managed_repository()
        .retain_consumer_catalog_grant(ctx.system_admin().id(), resource, &identity.consumer_id)
        .await?;
    Ok(())
}
