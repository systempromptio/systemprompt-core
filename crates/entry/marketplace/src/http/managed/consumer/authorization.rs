//! Device credential consumer routes, distinct from administrative
//! authorization.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::ManagedConsumerState;
use super::error::ConsumerHttpError;
use axum::http::{HeaderMap, StatusCode, header};
use systemprompt_identifiers::ManagedResourceId;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::feedback::receipts::AuthenticatedConsumerDevice;

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
    state: &ManagedConsumerState,
    credential: &str,
) -> Result<AuthenticatedConsumerDevice, ConsumerHttpError> {
    match state
        .repository
        .authenticate_consumer_device(credential)
        .await
    {
        Ok(identity) => Ok(identity),
        Err(ManagedError::Unavailable) => Err(ConsumerHttpError(StatusCode::UNAUTHORIZED)),
        Err(error) => Err(ConsumerHttpError::from(error)),
    }
}

#[must_use]
pub fn instance_serves(services: &ServicesConfig, host: EvaluatorClient) -> bool {
    services
        .instance_enabled_hosts()
        .iter()
        .any(|served| host.accepts(*served))
}

pub(super) async fn resource(
    state: &ManagedConsumerState,
    credential: &str,
    resource: &ManagedResourceId,
    host: EvaluatorClient,
) -> Result<(), ConsumerHttpError> {
    let identity = authenticate_device(state, credential).await?;
    if !instance_serves(&state.services.config(), host) {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    let skills = state
        .catalog
        .granted_skills(&identity.consumer_id)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "consumer: candidate assembly failed");
            ConsumerHttpError(StatusCode::SERVICE_UNAVAILABLE)
        })?;
    if !skills.iter().any(|skill| {
        skill
            .publication
            .as_ref()
            .is_some_and(|publication| &publication.resource_id == resource)
            && (skill.hosts.is_empty()
                || skill.hosts.iter().any(|value| {
                    value
                        .parse::<HostKind>()
                        .is_ok_and(|value| host.accepts(value))
                }))
    }) {
        return Err(ConsumerHttpError(StatusCode::FORBIDDEN));
    }
    let repository = &state.repository;
    let owner = repository.consumer_resource_owner(resource).await?;
    repository
        .retain_consumer_catalog_grant(&owner, resource, &identity.consumer_id)
        .await?;
    Ok(())
}
