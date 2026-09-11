//! The seam that makes catalog discovery a provider capability.
//!
//! Discovery began as one Vertex-shaped function. The parts that are actually
//! Vertex-shaped are narrow — which host counts, which credential can list it,
//! and the wire format of the listing call — and everything else (deciding
//! what is priced, what wins against an explicit declaration, what goes in the
//! report) is policy this deployment applies to any upstream that can be
//! asked what it serves.
//!
//! So a [`CatalogSource`] answers only the narrow questions, in a vocabulary
//! that carries no Google in it: given a provider and the credential its
//! secret parsed into, do I apply, and if so what does this upstream say it
//! serves? The next source — an Azure deployment listing, a self-hosted
//! `/v1/models` — is a new implementation and no change to the caller.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_models::services::ProviderEntry;
use systemprompt_security::credential::{AuthHeader, CredentialScope, ProviderCredential};
use thiserror::Error;

/// How far along an upstream considers a model to be.
///
/// Only two states matter to the pricing decision: a model an upstream calls
/// generally available, and everything else, which is served only where the
/// rate card explicitly opts in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchStage {
    GenerallyAvailable,
    Preview,
}

impl LaunchStage {
    #[must_use]
    pub const fn is_generally_available(self) -> bool {
        matches!(self, Self::GenerallyAvailable)
    }
}

/// One model an upstream says it serves, reduced to what the decision needs.
///
/// `upstream` is how the rate card names it (e.g. `google/gemini-2.5-pro`)
/// and `serverless` is whether it can be called without the operator
/// deploying anything first.
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub upstream: String,
    pub launch_stage: LaunchStage,
    pub serverless: bool,
}

/// What one source returned for one provider.
///
/// Failures travel beside the models rather than instead of them: a publisher
/// we are not entitled to answers 403, and that must not cost us the
/// publishers we are entitled to. Each string is already in the shape
/// [`DiscoveryReport::failed_publishers`](systemprompt_models::services::DiscoveryReport)
/// takes.
#[derive(Debug, Default)]
pub struct CatalogListing {
    pub models: Vec<DiscoveredModel>,
    pub failures: Vec<String>,
}

/// A listing that could not be attempted at all.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("{0}")]
    Unusable(String),
}

/// An upstream that can be asked which models it serves.
///
/// `name` labels the source in report lines and logs. `matches_provider` is
/// the cheap, credential-free question — could this source ever list this
/// provider? — kept separate from `applies` (the same question with the
/// secret parsed) so the caller knows whether a provider is worth parsing a
/// secret for before it parses one, and so a malformed secret on an unrelated
/// provider is not reported as a discovery failure. `list` asks the upstream
/// what it serves.
///
/// `#[async_trait]` because sources are held as `dyn CatalogSource`.
#[async_trait]
pub trait CatalogSource: Send + Sync {
    fn name(&self) -> &'static str;

    fn matches_provider(&self, provider: &ProviderEntry) -> bool;

    fn applies(&self, provider: &ProviderEntry, credential: &ProviderCredential) -> bool;

    async fn list(
        &self,
        http: &reqwest::Client,
        auth: &AuthHeader,
        provider: &ProviderEntry,
        scope: &CredentialScope,
    ) -> Result<CatalogListing, DiscoveryError>;
}
