//! AI-config validation: provider credentials and sampling ranges.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_models::services::AiConfig;
use tracing::warn;

use crate::error::Result;
use crate::services::providers::AiProvider;

#[derive(Debug, Copy, Clone)]
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(
        config: &AiConfig,
        providers: &HashMap<String, Arc<dyn AiProvider>>,
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
        providers: &HashMap<String, Arc<dyn AiProvider>>,
        missing_env_vars: &[String],
    ) -> Result<()> {
        if providers.is_empty() {
            return Err(crate::error::AiError::Internal(Self::no_providers_message(
                config,
                missing_env_vars,
            )));
        }

        let default = &config.default_provider;
        if !config.providers.get(default).is_some_and(|c| c.enabled) {
            return Err(crate::error::AiError::Internal(format!(
                "Default provider '{}' must be an enabled entry under ai.providers.\nEnabled \
                 policy providers: {:?}\nFix: enable '{}' or change 'default_provider'",
                default,
                config
                    .providers
                    .iter()
                    .filter(|(_, c)| c.enabled)
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>(),
                default
            )));
        }

        if !providers.contains_key(default) {
            return Err(crate::error::AiError::Internal(format!(
                "Default provider '{}' has no connectivity in the profile registry.\nProviders \
                 with connectivity: {:?}\nFix: add a `providers` registry entry named '{}'",
                default,
                providers.keys().collect::<Vec<_>>(),
                default
            )));
        }

        Ok(())
    }

    fn no_providers_message(config: &AiConfig, missing_env_vars: &[String]) -> String {
        let mut error_msg = String::from("No AI providers are enabled.\n\n");

        if missing_env_vars.is_empty() {
            error_msg.push_str(
                "To fix, enable a provider in your AI policy and declare its \
                                connectivity in the profile `providers` registry:\n\n",
            );
            error_msg.push_str("  ai:\n");
            error_msg.push_str("    default_provider: gemini\n");
            error_msg.push_str("    providers:\n");
            error_msg.push_str("      gemini:\n");
            error_msg.push_str("        enabled: true\n\n");
            error_msg.push_str("And add the matching credential to your secrets.json.\n");
        } else {
            error_msg.push_str("Providers with unresolved secrets:\n");
            for env_var_message in missing_env_vars {
                error_msg.push_str(&format!("  - {env_var_message}\n"));
            }
            error_msg.push_str("\nTo fix: add the required API keys to your secrets.json file\n");
        }

        error_msg.push_str(&format!(
            "\nProviders defined in AI policy: {:?}",
            config.providers.keys().collect::<Vec<_>>()
        ));

        error_msg
    }

    fn validate_sampling(config: &AiConfig) {
        if !config.sampling.enable_smart_routing && !config.sampling.fallback_enabled {
            warn!("Both smart routing and fallback are disabled");
        }
    }

    fn validate_mcp(config: &AiConfig) -> Result<()> {
        let resilience = &config.mcp.resilience;
        if resilience.connect_timeout_ms == 0 {
            return Err(crate::error::AiError::Internal(
                "MCP connect timeout must be greater than 0".to_owned(),
            ));
        }

        if resilience.request_timeout_ms == 0 {
            return Err(crate::error::AiError::Internal(
                "MCP execution timeout must be greater than 0".to_owned(),
            ));
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
