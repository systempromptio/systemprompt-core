use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ModelId, ProviderId, SecretName};

use super::error::{GatewayProfileError, GatewayResult};
use crate::services::ai::ModelPricing;

/// Reject gateway upstream endpoints that point at the local host or private
/// network ranges; an operator-configured endpoint pointing at
/// `169.254.169.254` or an internal service would otherwise turn the inference
/// proxy into an SSRF primitive. Delegates to the shared outbound-URL guard so
/// gateway, webhook, and authz destinations enforce one policy.
pub(super) fn validate_endpoint(label: &str, endpoint: &str) -> GatewayResult<()> {
    // SYSTEMPROMPT_TRUSTED_HTTP_HOSTS is the sealed-network opt-in for
    // known-internal hostnames like the air-gap mock; empty when unset, so
    // production deployments keep the strict loopback-only http rule.
    let trusted = crate::net::trusted_http_hosts_from_env();
    crate::net::validate_outbound_url_with_trust(endpoint, &trusted)
        .map(|_| ())
        .map_err(|e| GatewayProfileError::BlockedEndpoint {
            label: label.to_owned(),
            endpoint: endpoint.to_owned(),
            reason: e.to_string(),
        })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayCatalog {
    #[serde(default)]
    pub providers: Vec<GatewayProvider>,
    #[serde(default)]
    pub models: Vec<GatewayModel>,
}

impl GatewayCatalog {
    pub fn validate(&self) -> GatewayResult<()> {
        for model in &self.models {
            if model.id.as_str().is_empty() {
                return Err(GatewayProfileError::ModelEmptyId);
            }
            if !self.providers.iter().any(|p| p.name == model.provider) {
                return Err(GatewayProfileError::UnknownProvider {
                    model: model.id.as_str().to_owned(),
                    provider: model.provider.as_str().to_owned(),
                });
            }
        }
        for provider in &self.providers {
            if provider.name.as_str().is_empty() {
                return Err(GatewayProfileError::ProviderEmptyName);
            }
            if provider.endpoint.is_empty() {
                return Err(GatewayProfileError::ProviderEmptyEndpoint {
                    name: provider.name.as_str().to_owned(),
                });
            }
            validate_endpoint(
                &format!("provider '{}'", provider.name.as_str()),
                &provider.endpoint,
            )?;
        }
        Ok(())
    }

    pub fn find_provider(&self, name: &str) -> Option<&GatewayProvider> {
        self.providers.iter().find(|p| p.name.as_str() == name)
    }

    #[must_use]
    pub fn contains_model(&self, requested: &str) -> bool {
        self.models.iter().any(|m| {
            m.id.as_str() == requested || m.aliases.iter().any(|a| a.as_str() == requested)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayProvider {
    pub name: ProviderId,
    pub endpoint: String,
    pub api_key_secret: SecretName,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayModel {
    pub id: ModelId,
    pub provider: ProviderId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
}
