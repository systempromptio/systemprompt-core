//! The credential the gateway presents to an upstream provider.
//!
//! This module is a thin view over [`UpstreamTarget`], the seam the gateway
//! shares with the in-process AI service. A hop's credential decides whose key
//! is presented and who is billed: a platform credential's secret, or the
//! member's own key stored under a personal credential id. The target looks
//! the secret up,
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

use std::sync::Arc;

use systemprompt_ai::models::BillingSource;
use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_ai::{EndpointBinding, UpstreamCall, UpstreamTarget, UpstreamTargetError};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::secrets::Secrets;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedHop};
use systemprompt_security::credential::{
    CredentialError, CredentialScope, TokenCache, fill_endpoint,
};

use super::{DispatchError, GatewayError};

/// The caller a personal credential is served for: their stored key is the
/// only key such a hop may present.
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
    catalog: &ResolvedCatalog,
    hop: &ResolvedHop,
    owner: &KeyOwner,
    tokens: &Arc<TokenCache>,
    secrets: &Secrets,
) -> Result<(UpstreamCall, BillingSource), DispatchError> {
    let binding = EndpointBinding::of_hop(catalog, hop).map_err(pre_audit)?;
    let billing = BillingSource::of(&hop.credential);
    if !hop.credential.is_personal() {
        let target = UpstreamTarget::from_secrets(tokens, secrets, binding).map_err(pre_audit)?;
        let call = target.call().await.map_err(pre_audit)?;
        return Ok((call, billing));
    }
    let credential = &hop.credential.id;
    let key = owner
        .credentials
        .find_key(&owner.user_id, credential)
        .await
        .map_err(|e| {
            DispatchError::pre_audit(GatewayError::internal("personal key lookup failed", e))
        })?
        .ok_or_else(|| {
            DispatchError::pre_audit(GatewayError::PersonalKeyRequired {
                credential: credential.as_str().to_owned(),
            })
        })?;
    let target = UpstreamTarget::resolve_personal(tokens, binding, &key, &owner.user_id)
        .map_err(pre_audit)?;
    let call = target.call().await.map_err(pre_audit)?;
    Ok((call, billing))
}

fn pre_audit(error: UpstreamTargetError) -> DispatchError {
    DispatchError::pre_audit(error)
}
