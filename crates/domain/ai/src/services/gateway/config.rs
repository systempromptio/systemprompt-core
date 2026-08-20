//! YAML schema for the declarative gateway-policy baseline.
//!
//! A deployment commits a [`GatewayPolicyConfig`] at
//! `services/gateway/policies.yaml` declaring the gateway policies every
//! instance should boot with. The bootstrap loader parses this struct, hands
//! it to [`super::ingestion::GatewayPolicyIngestionService`], and the service
//! projects it into `ai_gateway_policies`.
//!
//! The contract is one-way (YAML → DB), mirroring the access-control
//! ingestion path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::spec::GatewayPolicySpec;
use crate::error::RepositoryError;

const fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyConfig {
    #[serde(default)]
    pub policies: Vec<GatewayPolicyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyEntry {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub spec: GatewayPolicySpec,
}

impl GatewayPolicyConfig {
    pub fn validate(&self) -> Result<(), RepositoryError> {
        let mut seen = std::collections::HashSet::with_capacity(self.policies.len());
        for (idx, policy) in self.policies.iter().enumerate() {
            if policy.name.trim().is_empty() {
                return Err(RepositoryError::InvalidData {
                    field: format!("policies[{idx}].name"),
                    reason: "policy name must not be empty".to_owned(),
                });
            }
            if !seen.insert(policy.name.as_str()) {
                return Err(RepositoryError::InvalidData {
                    field: format!("policies[{idx}].name"),
                    reason: format!("duplicate policy name '{}'", policy.name),
                });
            }
            let safety = &policy.spec.safety;
            if safety.scanners.iter().any(|s| s == "heuristic")
                && crate::services::gateway::safety::effective_phrases(&safety.heuristic).is_empty()
            {
                return Err(RepositoryError::InvalidData {
                    field: format!("policies[{idx}].spec.safety.heuristic"),
                    reason: "heuristic scanner is enabled but its effective phrase list is \
                             empty — set phrases/extra_phrases or remove the scanner"
                        .to_owned(),
                });
            }
        }
        Ok(())
    }
}
