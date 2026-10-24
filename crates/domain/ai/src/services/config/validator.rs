//! AI-config validation over the resolved catalog.
//!
//! Checks in-process connectivity and the sampling, MCP and history ranges.
//! The catalog's own references are validated when it is resolved, so
//! nothing here re-reads provider config.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::AiConfig;
use systemprompt_manifest::services::catalog::ResolvedCatalog;
use tracing::warn;

use super::error::AiConfigError;
use crate::error::Result;
use crate::services::core::{ProviderClients, WithheldEndpoint};

#[derive(Debug, Copy, Clone)]
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(
        config: &AiConfig,
        catalog: &ResolvedCatalog,
        clients: &ProviderClients,
        withheld: &[WithheldEndpoint],
    ) -> Result<()> {
        Self::validate_connectivity(catalog, clients, withheld)?;
        Self::validate_sampling(config);
        Self::validate_mcp(config)?;
        Self::validate_history(config);
        Ok(())
    }

    fn validate_connectivity(
        catalog: &ResolvedCatalog,
        clients: &ProviderClients,
        withheld: &[WithheldEndpoint],
    ) -> Result<()> {
        if clients.is_empty() {
            return Err(AiConfigError::NoProvidersEnabled {
                enabled_providers: catalog
                    .providers()
                    .filter(|p| p.enabled)
                    .map(|p| p.id.as_str().to_owned())
                    .collect(),
                withheld: withheld
                    .iter()
                    .map(|w| format!("{}: {}", w.endpoint, w.reason))
                    .collect(),
            }
            .into());
        }
        Ok(())
    }

    fn validate_sampling(config: &AiConfig) {
        if !config.sampling.enable_smart_routing && !config.sampling.fallback_enabled {
            warn!("Both smart routing and fallback are disabled");
        }
    }

    fn validate_mcp(config: &AiConfig) -> Result<()> {
        let resilience = &config.mcp.resilience;
        if resilience.connect_timeout_ms == 0 {
            return Err(AiConfigError::ZeroMcpConnectTimeout.into());
        }

        if resilience.request_timeout_ms == 0 {
            return Err(AiConfigError::ZeroMcpRequestTimeout.into());
        }

        if resilience.retry_attempts == 0 {
            warn!("MCP retry attempts set to 0, failures will not be retried");
        }

        Ok(())
    }

    fn validate_history(config: &AiConfig) {
        let days = config.history.retention_days;
        if days == 0 {
            warn!("History retention set to 0 days, history will not be retained");
        } else if days > 365 {
            warn!(retention_days = days, "History retention exceeds 365 days");
        }
    }
}
