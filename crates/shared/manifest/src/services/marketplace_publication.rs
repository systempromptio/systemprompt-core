//! Services-wide policy for managed publication: whether a publication gate
//! must admit a publication, and the cohort rings a consumer resolves through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_models::errors::ServicesValidationError;

/// The `marketplace:` block of the services manifest.
///
/// `rings` is ordered: a consumer resolves a managed resource through the
/// first ring it belongs to that has a cohort selection for that resource, and
/// through the base selection otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManagedMarketplaceConfig {
    #[serde(default)]
    pub publication: PublicationConfig,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rings: Vec<CohortRing>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationConfig {
    #[serde(default)]
    pub gate: PublicationGateConfig,
}

/// `required` refuses every gated publication while no publication gate is
/// registered; `on_adoption` extends gating from improvements to initial
/// adoptions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicationGateConfig {
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub on_adoption: bool,
}

/// One cohort a consumer may belong to: `dimension` is `role` or a registered
/// subject-attribute dimension slug.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CohortRing {
    pub dimension: String,
    pub value: String,
}

impl ManagedMarketplaceConfig {
    #[must_use]
    pub fn ring(&self, dimension: &str, value: &str) -> Option<&CohortRing> {
        self.rings
            .iter()
            .find(|ring| ring.dimension == dimension && ring.value == value)
    }

    pub fn validate(&self) -> Result<(), ServicesValidationError> {
        let mut seen = BTreeSet::new();
        for ring in &self.rings {
            if ring.dimension.trim().is_empty() || ring.value.trim().is_empty() {
                return Err(ServicesValidationError::invalid_field(
                    "marketplace.rings: dimension and value must be non-empty",
                ));
            }
            if ring.dimension == "user" {
                return Err(ServicesValidationError::invalid_field(
                    "marketplace.rings: 'user' is not a cohort dimension",
                ));
            }
            if !seen.insert((ring.dimension.as_str(), ring.value.as_str())) {
                return Err(ServicesValidationError::invalid_field(format!(
                    "marketplace.rings: ring {}={} is declared twice",
                    ring.dimension, ring.value
                )));
            }
        }
        Ok(())
    }
}
