//! The resolved shapes dispatch, pricing, the bridge profile and audit read.
//!
//! Every reference is already followed and every default already applied:
//! a [`ResolvedHop`] carries its endpoint's wire, URL and betas, its
//! provider's credential, the upstream model name and the final price, so a
//! consumer never re-derives any of them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};

use systemprompt_identifiers::{
    CredentialId, EndpointId, ModelId, ProviderId, RouteId, SecretName,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::anthropic::AnthropicBeta;
use systemprompt_wire::{Hosting, ModelLimits, WireProtocol};

use crate::services::ai::{ModelCapabilities, ModelGovernance, ModelPricing};
use crate::services::catalog::credentials::CredentialKind;
use crate::services::catalog::endpoint::EndpointRef;
use crate::services::gateway::SelectionStrategy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCredential {
    pub id: CredentialId,
    pub kind: CredentialKind,
    pub secret: Option<SecretName>,
    pub label: Option<String>,
}

impl ResolvedCredential {
    #[must_use]
    pub const fn is_personal(&self) -> bool {
        self.kind.is_personal()
    }

    #[must_use]
    pub const fn platform_secret(&self) -> Option<&SecretName> {
        match self.kind {
            CredentialKind::Platform => self.secret.as_ref(),
            CredentialKind::Personal => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub id: ProviderId,
    pub credential: CredentialId,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub user_agent: Option<String>,
    pub forward_headers: Vec<String>,
    pub extra_headers: HashMap<String, String>,
    pub governance: ModelGovernance,
    pub enabled: bool,
    pub capabilities: ModelCapabilities,
    pub endpoints: Vec<EndpointId>,
}

impl ResolvedProvider {
    #[must_use]
    pub fn forwards_header(&self, name: &str) -> bool {
        self.forward_headers
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedEndpoint {
    pub provider: ProviderId,
    pub id: EndpointId,
    pub wire: WireProtocol,
    pub url: String,
    pub betas: Option<BTreeSet<AnthropicBeta>>,
    pub region: Option<String>,
    pub price_multiplier: Option<f64>,
    pub hosting: Hosting,
}

impl ResolvedEndpoint {
    #[must_use]
    pub fn endpoint_ref(&self) -> EndpointRef {
        EndpointRef {
            provider: self.provider.clone(),
            endpoint: self.id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub id: ModelId,
    pub upstream_model: String,
    pub display_name: Option<String>,
    pub capabilities: ModelCapabilities,
    pub limits: ModelLimits,
    pub pricing: ModelPricing,
    pub governance: Option<ModelGovernance>,
    pub clients: Vec<ClientFamily>,
}

impl ResolvedModel {
    #[must_use]
    pub const fn is_advertised(&self) -> bool {
        !self.clients.is_empty()
    }

    #[must_use]
    pub fn is_advertised_to(&self, family: ClientFamily) -> bool {
        self.clients.contains(&family)
    }
}

/// One deployment of one model, every reference followed.
#[derive(Debug, Clone)]
pub struct ResolvedHop {
    pub model: ModelId,
    pub provider: ProviderId,
    pub endpoint: EndpointId,
    pub credential: ResolvedCredential,
    pub url: String,
    pub wire: WireProtocol,
    pub betas: Option<BTreeSet<AnthropicBeta>>,
    pub upstream_model: String,
    pub pricing: ModelPricing,
    pub governance: ModelGovernance,
    pub hosting: Hosting,
    pub region: Option<String>,
    pub clients: Vec<ClientFamily>,
    pub weight: Option<u32>,
}

impl ResolvedHop {
    #[must_use]
    pub fn endpoint_ref(&self) -> EndpointRef {
        EndpointRef {
            provider: self.provider.clone(),
            endpoint: self.endpoint.clone(),
        }
    }

    #[must_use]
    pub fn effective_weight(&self) -> u32 {
        self.weight.unwrap_or(1)
    }

    #[must_use]
    pub fn bills_same_credential(&self, other: &Self) -> bool {
        self.credential.id == other.credential.id
    }
}

/// What one request resolved to.
///
/// The catalog model, its ordered chain, the context-window fallbacks, and
/// the policy route that shaped it, if any. `access_route_id` is the route id
/// access control evaluates: the matched route's, else the catalog route id.
#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub requested: String,
    pub model: ResolvedModel,
    pub chain: Vec<ResolvedHop>,
    pub context_fallbacks: Vec<ResolvedHop>,
    pub strategy: SelectionStrategy,
    pub route: Option<RouteId>,
    pub access_route_id: RouteId,
}

impl ResolvedTarget {
    #[must_use]
    pub fn primary(&self) -> Option<&ResolvedHop> {
        self.chain.first()
    }
}
