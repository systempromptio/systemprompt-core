//! AI-config validation: provider credentials and sampling ranges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_manifest::services::AiConfig;
use tracing::warn;

use super::error::AiConfigError;
use crate::error::Result;
use crate::services::providers::ProviderClient;

#[derive(Debug, Copy, Clone)]
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(
        config: &AiConfig,
        providers: &HashMap<String, Arc<dyn ProviderClient>>,
        missing_env_vars: &[String],
    ) -> Result<()> {
        Self::validate_providers(config, providers, missing_env_vars)?;
        Self::validate_sampling(config);
        Self::validate_mcp(config)?;
        Self::validate_history(config);
        Ok(())
    }

    fn validate_providers(
        config: &AiConfig,
        providers: &HashMap<String, Arc<dyn ProviderClient>>,
        missing_env_vars: &[String],
    ) -> Result<()> {
        if providers.is_empty() {
            return Err(AiConfigError::NoProvidersEnabled {
                policy_providers: config.providers.keys().cloned().collect(),
                unresolved_secrets: missing_env_vars.to_vec(),
            }
            .into());
        }

        let default = &config.default_provider;
        if !config.providers.get(default).is_some_and(|c| c.enabled) {
            return Err(AiConfigError::DefaultProviderNotEnabled {
                provider: default.clone(),
                enabled: config
                    .providers
                    .iter()
                    .filter(|(_, c)| c.enabled)
                    .map(|(name, _)| name.clone())
                    .collect(),
            }
            .into());
        }

        if !providers.contains_key(default) {
            let needle = format!("Provider '{default}'");
            return Err(AiConfigError::DefaultProviderNoConnectivity {
                provider: default.clone(),
                connected: providers.keys().cloned().collect(),
                withheld: missing_env_vars
                    .iter()
                    .find(|m| m.contains(&needle))
                    .cloned(),
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
