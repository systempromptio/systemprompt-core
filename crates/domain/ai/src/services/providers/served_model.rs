//! [`ServedModel`]: one catalog model as one endpoint serves it.
//!
//! A provider client talks to one endpoint, so its model list is the
//! deployments on that endpoint: the catalog model id a caller names, the
//! upstream name the endpoint is sent, and the final price of that
//! deployment ([`ResolvedHop::pricing`], with every override and the
//! endpoint's price multiplier already applied). Nothing here re-derives a
//! price or a name from the model entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ModelId;
use systemprompt_manifest::services::ModelPricing;
use systemprompt_manifest::services::catalog::{EndpointRef, ResolvedCatalog, ResolvedHop};

#[derive(Debug, Clone, PartialEq)]
pub struct ServedModel {
    pub id: ModelId,
    pub upstream_model: String,
    pub pricing: ModelPricing,
}

impl ServedModel {
    #[must_use]
    pub fn from_hop(hop: &ResolvedHop) -> Self {
        Self {
            id: hop.model.clone(),
            upstream_model: hop.upstream_model.clone(),
            pricing: hop.pricing,
        }
    }

    #[must_use]
    pub fn matches(&self, requested: &str) -> bool {
        self.id.as_str() == requested || self.upstream_model == requested
    }
}

/// Every model deployed on `endpoint`, in catalog model order.
#[must_use]
pub fn served_models_on(catalog: &ResolvedCatalog, endpoint: &EndpointRef) -> Vec<ServedModel> {
    catalog
        .models()
        .filter_map(|model| {
            catalog
                .deployments(model.id.as_str())
                .iter()
                .find(|hop| &hop.endpoint_ref() == endpoint)
                .map(ServedModel::from_hop)
        })
        .collect()
}

/// The upstream name for `requested`, or `requested` itself when the endpoint
/// does not list it.
#[must_use]
pub fn upstream_model_in<'a>(models: &'a [ServedModel], requested: &'a str) -> &'a str {
    models
        .iter()
        .find(|model| model.matches(requested))
        .map_or(requested, |model| model.upstream_model.as_str())
}
