//! `services/ai/deployments.yaml`: which model is served where.
//!
//! Each model maps to an ordered list of endpoints, and the order is the
//! failover order. An entry is either a bare `provider/endpoint` or a block
//! that also sets the upstream name, a partial price override or the client
//! families for that one deployment. This file *is* routing for the common
//! case: dispatch resolves `model → deployment → endpoint → provider →
//! credential` with no glob matching.
//!
//! Price precedence for one hop: a deployment's `pricing` field wins, then the
//! endpoint's `price_multiplier` applied to the model's base price, then the
//! model's base price ([`PricingOverride::resolve`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ModelId;
use systemprompt_models::providers::ClientFamily;

use super::endpoint::EndpointRef;
use crate::services::ai::ModelPricing;

/// Per-field price override of one deployment; an absent field keeps the
/// multiplied base price.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PricingOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_per_million: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_per_million: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_per_million: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_per_million: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_image_cents: Option<f64>,
}

impl PricingOverride {
    #[must_use]
    pub fn resolve(
        override_: Option<&Self>,
        multiplier: Option<f64>,
        base: ModelPricing,
    ) -> ModelPricing {
        let scaled = multiplier.map_or(base, |factor| ModelPricing {
            input_per_million: base.input_per_million * factor,
            output_per_million: base.output_per_million * factor,
            cache_read_per_million: base.cache_read_per_million.map(|rate| rate * factor),
            cache_write_per_million: base.cache_write_per_million.map(|rate| rate * factor),
            per_image_cents: base.per_image_cents.map(|cents| cents * factor),
        });
        let Some(over) = override_ else {
            return scaled;
        };
        ModelPricing {
            input_per_million: over.input_per_million.unwrap_or(scaled.input_per_million),
            output_per_million: over.output_per_million.unwrap_or(scaled.output_per_million),
            cache_read_per_million: over
                .cache_read_per_million
                .or(scaled.cache_read_per_million),
            cache_write_per_million: over
                .cache_write_per_million
                .or(scaled.cache_write_per_million),
            per_image_cents: over.per_image_cents.or(scaled.per_image_cents),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeploymentDetail {
    pub endpoint: EndpointRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clients: Option<Vec<ClientFamily>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum DeploymentTarget {
    Endpoint(EndpointRef),
    Detailed(DeploymentDetail),
}

impl DeploymentTarget {
    #[must_use]
    pub const fn endpoint(&self) -> &EndpointRef {
        match self {
            Self::Endpoint(endpoint) => endpoint,
            Self::Detailed(detail) => &detail.endpoint,
        }
    }

    #[must_use]
    pub fn upstream_model(&self) -> Option<&str> {
        match self {
            Self::Endpoint(_) => None,
            Self::Detailed(detail) => detail.upstream_model.as_deref(),
        }
    }

    #[must_use]
    pub const fn pricing(&self) -> Option<&PricingOverride> {
        match self {
            Self::Endpoint(_) => None,
            Self::Detailed(detail) => detail.pricing.as_ref(),
        }
    }

    #[must_use]
    pub fn clients(&self) -> Option<&[ClientFamily]> {
        match self {
            Self::Endpoint(_) => None,
            Self::Detailed(detail) => detail.clients.as_deref(),
        }
    }
}

impl From<EndpointRef> for DeploymentTarget {
    fn from(endpoint: EndpointRef) -> Self {
        Self::Endpoint(endpoint)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct DeploymentRegistry {
    pub deployments: BTreeMap<ModelId, Vec<DeploymentTarget>>,
}

impl DeploymentRegistry {
    #[must_use]
    pub fn get(&self, model: &str) -> Option<&[DeploymentTarget]> {
        self.deployments.get(model).map(Vec::as_slice)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ModelId, &Vec<DeploymentTarget>)> {
        self.deployments.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.deployments.is_empty()
    }
}
