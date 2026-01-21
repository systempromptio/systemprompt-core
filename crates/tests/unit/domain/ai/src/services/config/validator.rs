//! Tests for ConfigValidator.

use systemprompt_ai::services::config::ConfigValidator;
use systemprompt_models::services::{AiConfig, AiProviderConfig, HistoryConfig, McpConfig, SamplingConfig};
use std::collections::HashMap;

fn create_valid_config() -> AiConfig {
    let mut providers = HashMap::new();
    providers.insert(
        "openai".to_string(),
        AiProviderConfig {
            enabled: true,
            api_key: "sk-test-key".to_string(),
            endpoint: None,
            default_model: "gpt-4".to_string(),
            google_search_enabled: false,
            models: HashMap::new(),
        },
    );

    AiConfig {
        default_provider: "openai".to_string(),
        default_max_output_tokens: Some(4096),
        providers,
        tool_models: HashMap::new(),
        sampling: SamplingConfig {
            enable_smart_routing: true,
            fallback_enabled: true,
        },
        mcp: McpConfig {
            auto_discover: false,
            connect_timeout_ms: 5000,
            execution_timeout_ms: 30000,
            retry_attempts: 3,
        },
        history: HistoryConfig {
            retention_days: 30,
            log_tool_executions: true,
        },
    }
}

mod validate_providers_tests {
    use super::*;

    #[test]
    fn valid_config_passes() {
        let config = create_valid_config();
        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn rejects_no_enabled_providers() {
        let mut config = create_valid_config();
        config.providers.get_mut("openai").unwrap().enabled = false;

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No AI providers"));
    }

    #[test]
    fn includes_missing_env_vars_in_error() {
        let mut config = create_valid_config();
        config.providers.get_mut("openai").unwrap().enabled = false;

        let missing = vec!["OPENAI_API_KEY not set".to_string()];
        let result = ConfigValidator::validate(&config, &missing);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn rejects_enabled_provider_without_api_key() {
        let mut config = create_valid_config();
        config.providers.get_mut("openai").unwrap().api_key = String::new();

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no API key"));
    }

    #[test]
    fn rejects_enabled_provider_without_default_model() {
        let mut config = create_valid_config();
        config.providers.get_mut("openai").unwrap().default_model = String::new();

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no default model"));
    }

    #[test]
    fn rejects_unknown_default_provider() {
        let mut config = create_valid_config();
        config.default_provider = "unknown".to_string();

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn rejects_disabled_default_provider() {
        let mut config = create_valid_config();

        // Add another enabled provider
        config.providers.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "sk-ant-key".to_string(),
                endpoint: None,
                default_model: "claude-3".to_string(),
                google_search_enabled: false,
                models: HashMap::new(),
            },
        );

        // Disable the default
        config.providers.get_mut("openai").unwrap().enabled = false;

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not enabled"));
    }

    #[test]
    fn accepts_multiple_enabled_providers() {
        let mut config = create_valid_config();
        config.providers.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "sk-ant-key".to_string(),
                endpoint: None,
                default_model: "claude-3".to_string(),
                google_search_enabled: false,
                models: HashMap::new(),
            },
        );

        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }
}

mod validate_mcp_tests {
    use super::*;

    #[test]
    fn rejects_zero_connect_timeout() {
        let mut config = create_valid_config();
        config.mcp.connect_timeout_ms = 0;

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("connect timeout"));
    }

    #[test]
    fn rejects_zero_execution_timeout() {
        let mut config = create_valid_config();
        config.mcp.execution_timeout_ms = 0;

        let result = ConfigValidator::validate(&config, &[]);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("execution timeout"));
    }

    #[test]
    fn accepts_zero_retry_attempts() {
        let mut config = create_valid_config();
        config.mcp.retry_attempts = 0;

        // Should pass but log a warning (we can't test the warning easily)
        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }
}

mod validate_sampling_tests {
    use super::*;

    #[test]
    fn accepts_both_routing_disabled() {
        let mut config = create_valid_config();
        config.sampling.enable_smart_routing = false;
        config.sampling.fallback_enabled = false;

        // Should pass but log a warning
        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }
}

mod validate_history_tests {
    use super::*;

    #[test]
    fn accepts_zero_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 0;

        // Should pass but log a warning
        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn accepts_high_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 500;

        // Should pass but log a warning
        let result = ConfigValidator::validate(&config, &[]);
        assert!(result.is_ok());
    }
}
