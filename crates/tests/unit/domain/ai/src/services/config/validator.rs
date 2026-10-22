//! Tests for ConfigValidator over the resolved catalog.

use crate::services::providers::mock_http;

use systemprompt_ai::services::config::ConfigValidator;
use systemprompt_ai::{ProviderClients, WithheldEndpoint};
use systemprompt_manifest::services::catalog::{EndpointRef, ResolvedCatalog};
use systemprompt_manifest::services::{AiConfig, HistoryConfig, McpConfig, SamplingConfig};
use systemprompt_test_fixtures::ensure_test_secrets_bootstrap;

fn create_valid_config() -> AiConfig {
    AiConfig {
        default_max_output_tokens: Some(4096),
        default_image_model: None,
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

fn catalog_and_clients() -> (ResolvedCatalog, ProviderClients) {
    ensure_test_secrets_bootstrap();
    let catalog = mock_http::default_catalog();
    let (clients, _withheld) = ProviderClients::build(&catalog).expect("clients build");
    (catalog, clients)
}

fn validate(config: &AiConfig) -> systemprompt_ai::error::Result<()> {
    let (catalog, clients) = catalog_and_clients();
    ConfigValidator::validate(config, &catalog, &clients, &[])
}

mod validate_connectivity_tests {
    use super::*;

    #[test]
    fn accepts_a_catalog_with_served_endpoints() {
        let (catalog, clients) = catalog_and_clients();
        assert!(
            !clients.is_empty(),
            "the default catalog's three endpoints have keys"
        );
        ConfigValidator::validate(&create_valid_config(), &catalog, &clients, &[])
            .expect("served endpoints pass");
    }

    #[test]
    fn rejects_no_usable_endpoint() {
        let catalog = mock_http::default_catalog();
        let err = ConfigValidator::validate(
            &create_valid_config(),
            &catalog,
            &ProviderClients::default(),
            &[],
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("No AI provider endpoint is usable"),
            "{message}"
        );
        assert!(
            message.contains("anthropic"),
            "names the enabled providers: {message}"
        );
    }

    #[test]
    fn names_every_withheld_endpoint_and_why() {
        let catalog = mock_http::default_catalog();
        let withheld = [WithheldEndpoint {
            endpoint: EndpointRef::new("openai", "api"),
            reason: "credential 'openai' secret 'openai' is not configured".to_owned(),
        }];
        let err = ConfigValidator::validate(
            &create_valid_config(),
            &catalog,
            &ProviderClients::default(),
            &withheld,
        )
        .unwrap_err();
        let message = err.to_string();
        assert!(message.contains("openai/api"), "{message}");
        assert!(message.contains("is not configured"), "{message}");
    }
}

mod validate_mcp_tests {
    use super::*;

    #[test]
    fn rejects_zero_connect_timeout() {
        let mut config = create_valid_config();
        config.mcp.resilience.connect_timeout_ms = 0;

        let err = validate(&config).unwrap_err();

        assert!(err.to_string().contains("connect timeout"));
    }

    #[test]
    fn rejects_zero_execution_timeout() {
        let mut config = create_valid_config();
        config.mcp.resilience.request_timeout_ms = 0;

        let err = validate(&config).unwrap_err();

        assert!(err.to_string().contains("execution timeout"));
    }

    #[test]
    fn accepts_zero_retry_attempts() {
        let mut config = create_valid_config();
        config.mcp.resilience.retry_attempts = 0;

        validate(&config).expect("zero retry attempts should pass");
    }
}

mod validate_sampling_tests {
    use super::*;

    #[test]
    fn accepts_both_routing_disabled() {
        let mut config = create_valid_config();
        config.sampling.enable_smart_routing = false;
        config.sampling.fallback_enabled = false;

        validate(&config).expect("disabled routing should pass");
    }
}

mod validate_history_tests {
    use super::*;

    #[test]
    fn accepts_zero_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 0;

        validate(&config).expect("zero retention should pass");
    }

    #[test]
    fn accepts_high_retention() {
        let mut config = create_valid_config();
        config.history.retention_days = 500;

        validate(&config).expect("high retention should pass");
    }
}
