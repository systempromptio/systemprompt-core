//! Tests for ConfigValidator.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_ai::services::config::ConfigValidator;
use systemprompt_ai::services::providers::AiProvider;
use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_models::services::{
    AiConfig, AiProviderConfig, HistoryConfig, McpConfig, SamplingConfig,
};

fn create_valid_config() -> AiConfig {
    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        AiProviderConfig {
            enabled: true,
            default_model: "gpt-4".to_string(),
            default_image_model: String::new(),
            google_search_enabled: false,
            ..AiProviderConfig::default()
        },
    );

    AiConfig {
        default_provider: "openai".to_string(),
        default_max_output_tokens: Some(4096),
        providers,
        sampling: SamplingConfig {
            enable_smart_routing: true,
            fallback_enabled: true,
        },
        mcp: McpConfig::default(),
        history: HistoryConfig {
            retention_days: 30,
            log_tool_executions: true,
        },
    }
}

/// Build the registry-backed providers map the validator checks for
/// connectivity. The validator only inspects map membership, never invokes the
/// trait, so a lightweight Anthropic client stands in for any provider.
fn built_providers(names: &[&str]) -> HashMap<String, Arc<dyn AiProvider>> {
    names
        .iter()
        .map(|name| {
            let provider: Arc<dyn AiProvider> = Arc::new(AnthropicProvider::with_endpoint(
                "k".to_string(),
                "http://localhost".to_string(),
            ));
            ((*name).to_string(), provider)
        })
        .collect()
}

mod validate_providers_tests {
    use super::*;

    #[test]
    fn valid_config_passes() {
        let config = create_valid_config();
        ConfigValidator::validate(&config, &built_providers(&["openai"]), &[])
            .expect("valid config should pass validation");
    }

    #[test]
    fn rejects_no_enabled_providers() {
        let config = create_valid_config();

        let err = ConfigValidator::validate(&config, &built_providers(&[]), &[]).unwrap_err();

        assert!(err.to_string().contains("No AI providers"));
    }

    #[test]
    fn includes_missing_env_vars_in_error() {
        let config = create_valid_config();

        let missing = vec!["OPENAI_API_KEY not set".to_string()];
        let error = ConfigValidator::validate(&config, &built_providers(&[]), &missing)
            .unwrap_err()
            .to_string();
        assert!(error.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn rejects_default_provider_without_connectivity() {
        let config = create_valid_config();

        let err =
            ConfigValidator::validate(&config, &built_providers(&["anthropic"]), &[]).unwrap_err();

        assert!(err.to_string().contains("no connectivity"));
    }

    #[test]
    fn rejects_unknown_default_provider() {
        let mut config = create_valid_config();
        config.default_provider = "unknown".to_string();

        let err =
            ConfigValidator::validate(&config, &built_providers(&["openai"]), &[]).unwrap_err();

        assert!(err.to_string().contains("must be an enabled entry"));
    }

    #[test]
    fn rejects_disabled_default_provider() {
        let mut config = create_valid_config();

        config.providers.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                default_model: "claude-3".to_string(),
                default_image_model: String::new(),
                google_search_enabled: false,
                ..AiProviderConfig::default()
            },
        );

        config.providers.get_mut("openai").unwrap().enabled = false;

        let err =
            ConfigValidator::validate(&config, &built_providers(&["anthropic"]), &[]).unwrap_err();

        assert!(err.to_string().contains("must be an enabled entry"));
    }

    #[test]
    fn accepts_multiple_enabled_providers() {
        let mut config = create_valid_config();
        config.providers.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                default_model: "claude-3".to_string(),
                default_image_model: String::new(),
                google_search_enabled: false,
                ..AiProviderConfig::default()
            },
        );

        ConfigValidator::validate(&config, &built_providers(&["openai", "anthropic"]), &[])
            .expect("multiple enabled providers should pass");
    }
}

mod validate_mcp_tests {
    use super::*;

    #[test]
    fn rejects_zero_connect_timeout() {
        let mut config = create_valid_config();
        config.mcp.resilience.connect_timeout_ms = 0;

        let err =
            ConfigValidator::validate(&config, &built_providers(&["openai"]), &[]).unwrap_err();

        assert!(err.to_string().contains("connect timeout"));
    }

    #[test]
    fn rejects_zero_execution_timeout() {
        let mut config = create_valid_config();
        config.mcp.resilience.request_timeout_ms = 0;

        let err =
            ConfigValidator::validate(&config, &built_providers(&["openai"]), &[]).unwrap_err();

        assert!(err.to_string().contains("execution timeout"));
    }

    #[test]
    fn accepts_zero_retry_attempts() {
        let mut config = create_valid_config();
        config.mcp.resilience.retry_attempts = 0;

        ConfigValidator::validate(&config, &built_providers(&["openai"]), &[])
            .expect("zero retry attempts should pass");
    }
}

mod validate_sampling_tests {
    use super::*;

    #[test]
    fn accepts_both_routing_disabled() {
        let mut config = create_valid_config();
        config.sampling.enable_smart_routing = false;
        config.sampling.fallback_enabled = false;

        ConfigValidator::validate(&config, &built_providers(&["openai"]), &[])
            .expect("disabled routing should pass");
    }
}

mod validate_history_tests {
    use super::*;

    #[test]
    fn accepts_zero_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 0;

        ConfigValidator::validate(&config, &built_providers(&["openai"]), &[])
            .expect("zero retention should pass");
    }

    #[test]
    fn accepts_high_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 500;

        ConfigValidator::validate(&config, &built_providers(&["openai"]), &[])
            .expect("high retention should pass");
    }
}
