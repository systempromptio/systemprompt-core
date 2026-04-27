use std::collections::HashMap;
use systemprompt_ai::services::providers::ProviderFactory;
use systemprompt_models::services::AiProviderConfig;

mod create_tests {
    use super::*;

    #[test]
    fn create_openai_provider_succeeds() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = ProviderFactory::create("openai", &config, None).expect("should succeed");
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn create_anthropic_provider_succeeds() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = ProviderFactory::create("anthropic", &config, None).expect("should succeed");
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn create_gemini_provider_succeeds() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = ProviderFactory::create("gemini", &config, None).expect("should succeed");
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn create_fails_when_provider_disabled() {
        let config = AiProviderConfig {
            enabled: false,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let error = ProviderFactory::create("openai", &config, None)
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("disabled"));
    }

    #[test]
    fn create_fails_for_unknown_provider() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let error = ProviderFactory::create("unknown-provider", &config, None)
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("Unknown provider"));
    }

    #[test]
    fn create_openai_with_custom_endpoint() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            endpoint: Some("https://custom.endpoint.com".to_string()),
            ..Default::default()
        };

        let result = ProviderFactory::create("openai", &config, None);
        result.expect("should succeed");
    }

    #[test]
    fn create_anthropic_with_custom_endpoint() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            endpoint: Some("https://custom.endpoint.com".to_string()),
            ..Default::default()
        };

        let result = ProviderFactory::create("anthropic", &config, None);
        result.expect("should succeed");
    }

    #[test]
    fn create_gemini_with_google_search_enabled() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: true,
            ..Default::default()
        };

        let provider = ProviderFactory::create("gemini", &config, None).expect("should succeed");
        assert!(provider.supports_google_search());
    }

    #[test]
    fn create_gemini_without_google_search() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: false,
            ..Default::default()
        };

        let provider = ProviderFactory::create("gemini", &config, None).expect("should succeed");
        assert!(!provider.supports_google_search());
    }
}

mod create_all_tests {
    use super::*;

    #[test]
    fn create_all_with_multiple_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "anthropic-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ProviderFactory::create_all(configs, None).expect("should succeed");
        assert_eq!(providers.len(), 2);
        assert!(providers.contains_key("openai"));
        assert!(providers.contains_key("anthropic"));
    }

    #[test]
    fn create_all_skips_disabled_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: false,
                api_key: "anthropic-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ProviderFactory::create_all(configs, None).expect("should succeed");
        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("openai"));
        assert!(!providers.contains_key("anthropic"));
    }

    #[test]
    fn create_all_fails_when_no_providers_enabled() {
        let mut configs = HashMap::new();

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: false,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        let error = ProviderFactory::create_all(configs, None)
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("No providers"));
    }

    #[test]
    fn create_all_fails_when_empty_config() {
        let configs = HashMap::new();

        ProviderFactory::create_all(configs, None)
            .err()
            .expect("should be an error");
    }

    #[test]
    fn create_all_with_all_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "anthropic".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "anthropic-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ProviderFactory::create_all(configs, None).expect("should succeed");
        assert_eq!(providers.len(), 3);
    }
}
