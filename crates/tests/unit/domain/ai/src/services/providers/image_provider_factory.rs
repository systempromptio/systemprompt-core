use std::collections::HashMap;
use systemprompt_ai::services::providers::ImageProviderFactory;
use systemprompt_models::services::AiProviderConfig;

mod create_tests {
    use super::*;

    #[test]
    fn create_gemini_image_provider_succeeds() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = ImageProviderFactory::create("gemini", &config)
            .expect("should succeed");
        assert_eq!(provider.name(), "gemini-image");
    }

    #[test]
    fn create_openai_image_provider_succeeds() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = ImageProviderFactory::create("openai", &config)
            .expect("should succeed");
        assert_eq!(provider.name(), "openai-image");
    }

    #[test]
    fn create_fails_when_disabled() {
        let config = AiProviderConfig {
            enabled: false,
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let error = ImageProviderFactory::create("gemini", &config)
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

        let error = ImageProviderFactory::create("unknown", &config)
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("Unknown image provider"));
    }

    #[test]
    fn create_gemini_with_custom_endpoint() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            endpoint: Some("https://custom.endpoint.com".to_string()),
            ..Default::default()
        };

        ImageProviderFactory::create("gemini", &config)
            .expect("should succeed");
    }

    #[test]
    fn create_openai_with_custom_endpoint() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            endpoint: Some("https://custom.endpoint.com".to_string()),
            ..Default::default()
        };

        ImageProviderFactory::create("openai", &config)
            .expect("should succeed");
    }

    #[test]
    fn create_gemini_with_default_model() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            default_model: "imagen-3.0-generate-001".to_string(),
            ..Default::default()
        };

        let provider = ImageProviderFactory::create("gemini", &config)
            .expect("should succeed");
        assert_eq!(provider.default_model(), "gemini-2.5-flash-image");
    }

    #[test]
    fn create_openai_with_default_model() {
        let config = AiProviderConfig {
            enabled: true,
            api_key: "test-key".to_string(),
            default_model: "dall-e-3".to_string(),
            ..Default::default()
        };

        let provider = ImageProviderFactory::create("openai", &config)
            .expect("should succeed");
        assert_eq!(provider.default_model(), "gpt-image-1");
    }
}

mod create_all_tests {
    use super::*;

    #[test]
    fn create_all_with_multiple_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ImageProviderFactory::create_all(&configs)
            .expect("should succeed");
        assert_eq!(providers.len(), 2);
        assert!(providers.contains_key("gemini"));
        assert!(providers.contains_key("openai"));
    }

    #[test]
    fn create_all_skips_disabled_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "openai".to_string(),
            AiProviderConfig {
                enabled: false,
                api_key: "openai-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ImageProviderFactory::create_all(&configs)
            .expect("should succeed");
        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("gemini"));
    }

    #[test]
    fn create_all_fails_when_no_providers_enabled() {
        let mut configs = HashMap::new();

        configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: false,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        let error = ImageProviderFactory::create_all(&configs)
            .err()
            .expect("should be an error");
        assert!(error.to_string().contains("No image providers"));
    }

    #[test]
    fn create_all_skips_unknown_providers() {
        let mut configs = HashMap::new();

        configs.insert(
            "gemini".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "gemini-key".to_string(),
                ..Default::default()
            },
        );

        configs.insert(
            "unknown".to_string(),
            AiProviderConfig {
                enabled: true,
                api_key: "unknown-key".to_string(),
                ..Default::default()
            },
        );

        let providers = ImageProviderFactory::create_all(&configs)
            .expect("should succeed");
        assert_eq!(providers.len(), 1);
        assert!(providers.contains_key("gemini"));
    }
}
