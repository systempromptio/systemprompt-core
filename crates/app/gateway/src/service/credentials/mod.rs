//! The credential the gateway presents to an upstream provider.
//!
//! This module is a thin view over [`UpstreamTarget`], the seam the gateway
//! shares with the in-process AI service: the target looks the secret up,
//! parses it into a credential, fills the endpoint from the credential's
//! scope and mints the auth header. It decides nothing about credential types
//! or hosting itself, which is the point — a new credential kind is a new
//! variant in the security crate, a new platform a new arm in the dialect,
//! and no change here at all.
//!
//! For almost every provider the stored secret *is* the credential and is sent
//! verbatim. Google Vertex AI is the exception that forced the model to exist:
//! its endpoints reject API-key authentication as a class — "API keys are not
//! supported by this API", HTTP 401 `CREDENTIALS_MISSING` — and want an OAuth
//! access token that asserts a principal, minted from a service-account key
//! and expiring in an hour.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod google;

use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_ai::{UpstreamCall, UpstreamTarget, UpstreamTargetError};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::ProviderEntry;
use systemprompt_security::credential::{CredentialError, CredentialScope, fill_endpoint};

use super::{DispatchError, GatewayError};

/// Whose account an upstream call bills, recorded as
/// `ai_requests.billing_source`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingSource {
    Platform,
    UserKey,
}

impl BillingSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::UserKey => "user_key",
        }
    }
}

/// The caller a `credential: user` provider is served for: their stored key
/// is the only credential such a provider may present.
#[derive(Debug, Clone)]
pub struct KeyOwner {
    pub credentials: UserProviderCredentialRepository,
    pub user_id: UserId,
}

pub fn fill_project(endpoint: &str, project: Option<&str>) -> Result<String, CredentialError> {
    let scope = CredentialScope {
        project: project.map(str::to_owned),
        ..CredentialScope::empty()
    };
    fill_endpoint(endpoint, &scope)
}

pub async fn resolve(
    provider: &ProviderEntry,
    owner: &KeyOwner,
) -> Result<(UpstreamCall, BillingSource), DispatchError> {
    if !provider.is_user_credential() {
        let target = UpstreamTarget::from_secrets(provider).map_err(pre_audit)?;
        let call = target.call().await.map_err(pre_audit)?;
        return Ok((call, BillingSource::Platform));
    }
    let name = provider.name.as_str();
    let key = owner
        .credentials
        .find_key(&owner.user_id, name)
        .await
        .map_err(|e| {
            DispatchError::pre_audit(GatewayError::internal("personal key lookup failed", e))
        })?
        .ok_or_else(|| {
            DispatchError::pre_audit(GatewayError::PersonalKeyRequired {
                provider: name.to_owned(),
            })
        })?;
    let target =
        UpstreamTarget::resolve_personal(provider, &key, &owner.user_id).map_err(pre_audit)?;
    let call = target.call().await.map_err(pre_audit)?;
    Ok((call, BillingSource::UserKey))
}

fn pre_audit(error: UpstreamTargetError) -> DispatchError {
    DispatchError::pre_audit(error)
}
