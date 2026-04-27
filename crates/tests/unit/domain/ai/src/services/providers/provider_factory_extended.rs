use std::collections::HashMap;
use systemprompt_ai::services::providers::{ImageProviderFactory, ProviderFactory};
use systemprompt_models::services::AiProviderConfig;

mod web_search_enablement {
    use super::*;

    #[test]
    fn openai_with_google_search_enabled() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: true,
            ..Default::default()
        };

        let provider =
            ProviderFactory::create("openai", &config, None).expect("should create provider");
        assert_eq!(provider.name(), "openai");
        assert!(provider.supports_google_search());
    }

    #[test]
    fn anthropic_with_google_search_enabled() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: true,
            ..Default::default()
        };

        let provider =
            ProviderFactory::create("anthropic", &config, None).expect("should create provider");
        assert_eq!(provider.name(), "anthropic");
        assert!(provider.supports_google_search());
    }

    #[test]
    fn openai_without_google_search() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: false,
            ..Default::default()
        };

        let provider =
            ProviderFactory::create("openai", &config, None).expect("should create provider");
        assert!(!provider.supports_google_search());
    }

    #[test]
    fn anthropic_without_google_search() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            google_search_enabled: false,
            ..Default::default()
        };

        let provider =
            ProviderFactory::create("anthropic", &config, None).expect("should create provider");
        assert!(!provider.supports_google_search());
    }
}

mod gemini_custom_endpoint {
    use super::*;

    #[test]
    fn create_gemini_with_custom_endpoint() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            endpoint: Some("https://custom-gemini.example.com".to_string()),
            ..Default::default()
        };

        let provider =
            ProviderFactory::create("gemini", &config, None).expect("should create provider");
        assert_eq!(provider.name(), "gemini");
    }
}

mod create_all_edge_cases {
    use super::*;

    #[test]
    fn create_all_ignores_unknown_provider_names() {
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
            "custom-llm".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "custom-key".to_string(),
                ..Default::default()
            },
        );

        let providers =
            ProviderFactory::create_all(configs, None).expect("should succeed with known provider");
        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("openai"));
    }

    #[test]
    fn create_all_with_only_unknown_providers_fails() {
        let mut configs = HashMap::new();

        configs.insert(
            "custom-llm".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "key".to_string(),
                ..Default::default()
            },
        );

        let error = ProviderFactory::create_all(configs, None)
            .err()
            .expect("should fail");
        assert!(error.to_string().contains("No providers"));
    }

    #[test]
    fn create_all_with_mixed_enabled_disabled() {
        let mut configs = HashMap::new();

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: false,
                api_key: "openai-key".to_string(),
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
        assert!(providers.contains_key("gemini"));
    }
}

mod image_provider_factory_supports {
    use super::*;

    #[test]
    fn supports_image_generation_for_openai() {
        assert!(ImageProviderFactory::supports_image_generation("openai"));
    }

    #[test]
    fn supports_image_generation_for_gemini() {
        assert!(ImageProviderFactory::supports_image_generation("gemini"));
    }

    #[test]
    fn does_not_support_image_generation_for_anthropic() {
        assert!(!ImageProviderFactory::supports_image_generation(
            "anthropic"
        ));
    }

    #[test]
    fn does_not_support_image_generation_for_unknown() {
        assert!(!ImageProviderFactory::supports_image_generation("unknown"));
    }
}

mod image_provider_factory_fallback {
    use super::*;

    #[test]
    fn fallback_succeeds_with_available_openai() {
        let anthropic_config = AiProviderConfig {
            enabled: true,
            api_key: "anthropic-key".to_string(),
            ..Default::default()
        };

        let mut all_configs = HashMap::new();
        all_configs.insert("anthropic".to_string(), anthropic_config.clone());
        all_configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        let provider = ImageProviderFactory::create_with_fallback(
            "anthropic",
            &anthropic_config,
            &all_configs,
        )
        .expect("should fallback to openai");
        assert_eq!(provider.name(), "openai-image");
    }

    #[test]
    fn fallback_succeeds_with_available_gemini() {
        let anthropic_config = AiProviderConfig {
            enabled: true,
            api_key: "anthropic-key".to_string(),
            ..Default::default()
        };

        let mut all_configs = HashMap::new();
        all_configs.insert("anthropic".to_string(), anthropic_config.clone());
        all_configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        let provider = ImageProviderFactory::create_with_fallback(
            "anthropic",
            &anthropic_config,
            &all_configs,
        )
        .expect("should fallback to gemini");
        assert!(provider.name() == "openai-image" || provider.name() == "gemini-image");
    }

    #[test]
    fn fallback_fails_when_no_image_providers_available() {
        let anthropic_config = AiProviderConfig {
            enabled: true,
            api_key: "anthropic-key".to_string(),
            ..Default::default()
        };

        let mut all_configs = HashMap::new();
        all_configs.insert("anthropic".to_string(), anthropic_config.clone());

        let error = ImageProviderFactory::create_with_fallback(
            "anthropic",
            &anthropic_config,
            &all_configs,
        )
        .err()
        .expect("should fail");
        assert!(error.to_string().contains("No image provider available"));
    }

    #[test]
    fn no_fallback_needed_when_primary_supports_images() {
        let gemini_config = AiProviderConfig {
            enabled: true,
            api_key: "gemini-key".to_string(),
            ..Default::default()
        };

        let all_configs = HashMap::new();

        let provider =
            ImageProviderFactory::create_with_fallback("gemini", &gemini_config, &all_configs)
                .expect("should use primary");
        assert_eq!(provider.name(), "gemini-image");
    }
}
