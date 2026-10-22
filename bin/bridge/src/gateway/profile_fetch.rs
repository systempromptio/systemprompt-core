//! `GET /v1/bridge/profile`, always with a bearer.
//!
//! The profile lists the caller's own personal-key models and entitlements,
//! so it is never fetched anonymously, and a 401 is an error, not a cue to
//! retry without a bearer. A 401 on a bearer old enough to have been voided
//! by a redeploy or key rotation earns exactly one re-mint
//! ([`super::rejection`], the rule the proxy uses too); a 401 on a fresh
//! bearer, or on the re-minted one, is returned as it is. The caller caches
//! the result per user.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::{Duration, Instant};

use super::rejection::rejection_is_stale;
use crate::gateway::errors::GatewayError;
use crate::gateway::types::BridgeProfile;
use crate::gateway::{GatewayClient, ensure_success, record_span};
use crate::ids::BearerToken;

/// A bearer and how long ago it was minted.
#[derive(Debug, Clone)]
pub struct AgedBearer {
    pub token: BearerToken,
    pub age: Duration,
}

impl AgedBearer {
    #[must_use]
    pub const fn fresh(token: BearerToken) -> Self {
        Self {
            token,
            age: Duration::ZERO,
        }
    }
}

/// A served profile and the bearer it was served to.
#[derive(Debug, Clone)]
pub struct ServedProfile {
    pub profile: BridgeProfile,
    pub bearer: BearerToken,
}

#[must_use]
pub const fn is_unauthorized(error: &GatewayError) -> bool {
    matches!(
        error,
        GatewayError::Rejected {
            status: reqwest::StatusCode::UNAUTHORIZED,
            ..
        }
    )
}

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "profile", status, latency_ms)
    )]
    pub async fn fetch_bridge_profile(
        &self,
        bearer: &BearerToken,
    ) -> Result<BridgeProfile, GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .get(self.url("/v1/bridge/profile"))
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::ProfileFetch(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "profile").await?;
        resp.json::<BridgeProfile>()
            .await
            .map_err(|e| GatewayError::ProfileDecode(Box::new(e)))
    }

    pub async fn fetch_bridge_profile_as_user<F, Fut, E>(
        &self,
        bearer: AgedBearer,
        remint: F,
    ) -> Result<ServedProfile, E>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<BearerToken, E>> + Send,
        E: From<GatewayError>,
    {
        match self.fetch_bridge_profile(&bearer.token).await {
            Ok(profile) => Ok(ServedProfile {
                profile,
                bearer: bearer.token,
            }),
            Err(error) if is_unauthorized(&error) && rejection_is_stale(bearer.age) => {
                tracing::info!(
                    age_secs = bearer.age.as_secs(),
                    "gateway refused a stale bearer for the bridge profile; re-minting once"
                );
                let fresh = remint().await?;
                let profile = self.fetch_bridge_profile(&fresh).await?;
                Ok(ServedProfile {
                    profile,
                    bearer: fresh,
                })
            },
            Err(error) => Err(error.into()),
        }
    }
}
