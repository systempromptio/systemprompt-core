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

use anyhow::anyhow;
use systemprompt_ai::{UpstreamCall, UpstreamTarget, UpstreamTargetError};
use systemprompt_models::services::ProviderEntry;
use systemprompt_security::credential::{CredentialError, CredentialScope, fill_endpoint};

use super::DispatchError;

pub fn fill_project(endpoint: &str, project: Option<&str>) -> Result<String, CredentialError> {
    let scope = CredentialScope {
        project: project.map(str::to_owned),
        ..CredentialScope::empty()
    };
    fill_endpoint(endpoint, &scope)
}

pub(super) async fn resolve(provider: &ProviderEntry) -> Result<UpstreamCall, DispatchError> {
    let target = UpstreamTarget::from_secrets(provider).map_err(pre_audit)?;
    target.call().await.map_err(pre_audit)
}

fn pre_audit(error: UpstreamTargetError) -> DispatchError {
    DispatchError::PreAudit(anyhow!(error))
}
