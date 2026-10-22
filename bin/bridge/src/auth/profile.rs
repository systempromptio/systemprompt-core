//! The bridge profile as the signed-in user, cached per `(gateway, user)`.
//!
//! The bearer is the cached credential for the gateway, or a freshly minted
//! one when none is cached; a stale one that the gateway refuses is cleared
//! and re-minted once (`gateway::profile_fetch`). The served profile is
//! remembered under the user its bearer names; a bearer that names no user is
//! served but not cached, so the cache never holds an anonymous entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use crate::config;
use crate::gateway::GatewayClient;
use crate::gateway::errors::GatewayError;
use crate::gateway::profile_fetch::{AgedBearer, ServedProfile};
use crate::gateway::types::BridgeProfile;
use crate::ids::BearerToken;

#[derive(Debug, thiserror::Error)]
pub enum ProfileFetchError {
    #[error(transparent)]
    Gateway(#[from] GatewayError),
    #[error("sign in to fetch the bridge profile: {0}")]
    Credential(#[from] super::ChainError),
    #[error("credential cache: {0}")]
    Cache(#[source] std::io::Error),
    #[error(transparent)]
    Config(#[from] config::ConfigReadError),
}

pub async fn fetch_as_user<F, Fut>(
    client: &GatewayClient,
    bearer: AgedBearer,
    remint: F,
) -> Result<BridgeProfile, ProfileFetchError>
where
    F: FnOnce() -> Fut + Send,
    Fut: Future<Output = Result<BearerToken, ProfileFetchError>> + Send,
{
    let ServedProfile { profile, bearer } =
        client.fetch_bridge_profile_as_user(bearer, remint).await?;
    let Some(user) = super::jwt::decode_unverified(bearer.as_str()).and_then(|id| id.user_id)
    else {
        tracing::warn!("the bearer names no user; the bridge profile is not cached");
        return Ok(profile);
    };
    if let Err(error) = super::profile_cache::remember(client.base_url().as_str(), &user, &profile)
    {
        tracing::warn!(%error, "could not cache the bridge profile");
    }
    Ok(profile)
}

pub async fn fetch_signed_in(client: &GatewayClient) -> Result<BridgeProfile, ProfileFetchError> {
    let cfg = config::load()?;
    let http = client.http().clone();
    let bearer = match super::cache::read_aged(&cfg, client.base_url(), 30)
        .map_err(ProfileFetchError::Cache)?
    {
        Some(aged) => AgedBearer {
            token: aged.output.token,
            age: aged.age,
        },
        None => AgedBearer::fresh(mint(&cfg, &http).await?),
    };
    fetch_as_user(client, bearer, || async move {
        super::cache::clear().map_err(ProfileFetchError::Cache)?;
        mint(&cfg, &http).await
    })
    .await
}

async fn mint(
    cfg: &config::Config,
    http: &reqwest::Client,
) -> Result<BearerToken, ProfileFetchError> {
    let session_id = super::session_identity::stable_session_id(cfg);
    Ok(super::mint_fresh(cfg, &session_id, http).await?.token)
}
