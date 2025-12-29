use anyhow::{anyhow, Result};
use systemprompt_models::services::AiConfig;
use tracing::warn;

#[derive(Debug, Copy, Clone)]
pub struct ConfigValidator;

impl ConfigValidator {
    pub fn validate(config: &AiConfig, missing_env_vars: &[String]) -> Result<()> {
        Self::validate_providers(config, missing_env_vars)?;
        Self::validate_sampling(config);
        Self::validate_mcp(config)?;
        Self::validate_history(config);
        Ok(())
    }

    fn validate_providers(config: &AiConfig, missing_env_vars: &[String]) -> Result<()> {
        let enabled_providers: Vec<_> =
            config.providers.iter().filter(|(_, c)| c.enabled).collect();

        if enabled_providers.is_empty() {
            let mut error_msg =
                String::from("No AI providers are enabled. Check your config file:\n\n");

            if missing_env_vars.is_empty() {
                error_msg.push_str("- Ensure at least one provider has 'enabled: true'\n");
                error_msg.push_str(
                    "- Verify API keys are set (GEMINI_API_KEY, ANTHROPIC_API_KEY, or \
                     OPENAI_API_KEY in .env)\n",
                );
            } else {
                error_msg.push_str("Providers disabled due to missing environment variables:\n");
                for env_var_message in missing_env_vars {
                    error_msg.push_str(&format!("  - {env_var_message}\n"));
                }
                error_msg.push_str("\nTo fix: Set the required API keys in your .env file\n");
            }

            error_msg.push_str(&format!(
                "\nCurrent providers defined: {:?}",
                config.providers.keys().collect::<Vec<_>>()
            ));

            return Err(anyhow!(error_msg));
        }

        for (name, provider_config) in &enabled_providers {
            if provider_config.api_key.is_empty() {
                return Err(anyhow!(
                    "Provider '{}' is enabled but has no API key.\nFix: Set {}_API_KEY in your \
                     .env file",
                    name,
                    name.to_uppercase()
                ));
            }

            if provider_config.default_model.is_empty() {
                return Err(anyhow!("Provider {name} has no default model specified"));
            }
        }

        if !config.providers.contains_key(&config.default_provider) {
            return Err(anyhow!(
                "Default provider '{}' not found in providers.\nAvailable providers: {:?}\nFix: \
                 Update 'default_provider' in your config file",
                config.default_provider,
                config.providers.keys().collect::<Vec<_>>()
            ));
        }

        if !config.providers[&config.default_provider].enabled {
            let available: Vec<&str> = config
                .providers
                .iter()
                .filter(|(_, c)| c.enabled)
                .map(|(n, _)| n.as_str())
                .collect();

            return Err(anyhow!(
                "Default provider '{}' is not enabled.\nEnabled providers: {:?}\nFix: Either \
                 enable '{}' in your config OR change 'default_provider' to one of the enabled \
                 providers",
                config.default_provider,
                available,
                config.default_provider
            ));
        }

        Ok(())
    }

    fn validate_sampling(config: &AiConfig) {
        if !config.sampling.enable_smart_routing && !config.sampling.fallback_enabled {
            warn!("Both smart routing and fallback are disabled");
        }
    }

    fn validate_mcp(config: &AiConfig) -> Result<()> {
        if config.mcp.connect_timeout_ms == 0 {
            return Err(anyhow!("MCP connect timeout must be greater than 0"));
        }

        if config.mcp.execution_timeout_ms == 0 {
            return Err(anyhow!("MCP execution timeout must be greater than 0"));
        }

        if config.mcp.retry_attempts == 0 {
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
