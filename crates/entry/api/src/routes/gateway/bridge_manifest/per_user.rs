//! Per-consumer inputs to the signed manifest: the user row, revoked
//! credentials, host preferences, and the catalogue grants the manifest
//! records for the organisation skills it carries.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::StatusCode;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_marketplace::MarketplaceCandidate;
use systemprompt_models::bridge::manifest::UserInfo;
use systemprompt_runtime::AppContext;

use super::super::bridge_data;

// Why: the recorded grant is the consumer's reach — a manifest whose grants
// could not be written must not serve the skills it would grant.
pub(super) async fn record_catalog_grants(
    ctx: &AppContext,
    user_id: &UserId,
    candidate: &MarketplaceCandidate,
) -> Result<(), (StatusCode, String)> {
    let owner = ctx.system_admin().id();
    if user_id == owner {
        return Ok(());
    }
    let repository = ctx.managed_repository();
    for publication in candidate
        .skills
        .iter()
        .filter_map(|skill| skill.publication.as_ref())
    {
        repository
            .retain_consumer_catalog_grant(owner, &publication.resource_id, user_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    %error,
                    resource = %publication.resource_id,
                    "manifest: recording catalogue grant failed"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("manifest: catalogue grant not recorded: {error}"),
                )
            })?;
    }
    Ok(())
}

pub(super) struct PerUserContext {
    pub user: Option<UserInfo>,
    pub revocations: Vec<ApiKeyId>,
    pub enabled_hosts: Vec<String>,
    pub host_model_protocols: std::collections::BTreeMap<String, Vec<String>>,
}

// Why: `revocations` and `enabled_hosts` are the policy half of the signed
// manifest — a failed read must not be served as "nothing revoked" / "every
// host enabled". Only the display and preference inputs may degrade.
pub(super) async fn load_per_user_context(
    ctx: &AppContext,
    user_id: &UserId,
    instance_hosts: Vec<String>,
) -> Result<PerUserContext, (StatusCode, String)> {
    let user = match bridge_data::load_user(ctx, user_id).await {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, "manifest: user load failed; continuing without user");
            None
        },
    };

    let revocations = bridge_data::load_revocations(ctx, user_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, "manifest: revocation load failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("manifest: revocations unavailable: {error}"),
            )
        })?;

    let enabled_hosts = match bridge_data::load_enabled_hosts(ctx, user_id).await {
        Ok(rows) if rows.is_empty() => instance_hosts,
        Ok(rows) => instance_hosts
            .into_iter()
            .filter(|h| rows.iter().any(|r| r == h))
            .collect(),
        Err(error) => {
            tracing::error!(%error, "manifest: enabled_hosts load failed");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("manifest: enabled hosts unavailable: {error}"),
            ));
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

    Ok(PerUserContext {
        user,
        revocations,
        enabled_hosts,
        host_model_protocols,
    })
}
