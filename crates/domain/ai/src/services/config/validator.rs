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
            let mut error_msg = String::from("No AI providers are enabled.\n\n");

            if missing_env_vars.is_empty() {
                error_msg.push_str("To fix, configure AI providers in your services.yaml:\n\n");
                error_msg.push_str("  ai:\n");
                error_msg.push_str("    default_provider: gemini\n");
                error_msg.push_str("    providers:\n");
                error_msg.push_str("      gemini:\n");
                error_msg.push_str("        enabled: true\n");
                error_msg.push_str("        api_key: \"${GEMINI_API_KEY}\"\n");
                error_msg.push_str("        default_model: gemini-2.5-flash-lite\n\n");
                error_msg.push_str("And add the API key to your secrets.json:\n\n");
                error_msg.push_str("  { \"gemini\": \"your-api-key-here\" }\n\n");
                error_msg.push_str("Supported providers: gemini, anthropic, openai\n");
            } else {
                error_msg.push_str("Providers disabled due to missing secrets:\n");
                for env_var_message in missing_env_vars {
                    error_msg.push_str(&format!("  - {env_var_message}\n"));
                }
                error_msg
                    .push_str("\nTo fix: Add the required API keys to your secrets.json file\n");
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
                    "Provider '{}' is enabled but has no API key.\n\nFix: Add '\"{}\"' key to \
                     your secrets.json file",
                    name,
                    name
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
