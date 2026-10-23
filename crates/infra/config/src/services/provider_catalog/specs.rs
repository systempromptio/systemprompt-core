//! What one `admin config catalog` upsert declares, per catalog object.
//!
//! A spec carries only the fields the operator states. Every `Option` that is
//! `None` keeps the stored value, so re-declaring a provider or endpoint never
//! drops its credential, user agent, headers, betas, region, price multiplier
//! or resilience. A new provider carries its first endpoint
//! (`ProviderSpec::first_endpoint`), since a provider without one does not
//! resolve.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::{CredentialId, EndpointId, ModelId, ProviderId};
use systemprompt_manifest::services::ModelPricing;
use systemprompt_manifest::services::catalog::{CredentialEntry, DeploymentTarget};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

#[derive(Debug, Clone)]
pub struct CredentialSpec {
    pub id: CredentialId,
    pub entry: CredentialEntry,
}

#[derive(Debug, Clone)]
pub struct ProviderSpec {
    pub id: ProviderId,
    pub credential: CredentialId,
    pub display_name: Option<String>,
    pub user_agent: Option<String>,
    pub extra_headers: HashMap<String, String>,
    pub enabled: Option<bool>,
    pub first_endpoint: Option<EndpointSpec>,
}

#[derive(Debug, Clone)]
pub struct EndpointSpec {
    pub provider: ProviderId,
    pub id: EndpointId,
    pub wire: WireProtocol,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub id: ModelId,
    pub upstream_model: Option<String>,
    pub display_name: Option<String>,
    pub clients: Option<Vec<ClientFamily>>,
    pub deployments: Vec<DeploymentTarget>,
    pub pricing: Option<ModelPricing>,
}
