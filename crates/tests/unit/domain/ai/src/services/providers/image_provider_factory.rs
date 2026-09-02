use systemprompt_ai::services::providers::{ImageProviderFactory, ImageProviderParams};
use systemprompt_models::profile::{ProviderEntry, ProviderRegistry, WireProtocol};
use systemprompt_models::services::AiProviderConfig;

fn entry(name: &str) -> ProviderEntry {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(name)
        .expect("provider present in default catalog")
        .clone()
}

mod create_tests {
    use super::*;

    #[test]
    fn create_gemini_image_provider_succeeds() {
        let e = entry("gemini");
        let policy = AiProviderConfig::default();
        let provider = ImageProviderFactory::create(&ImageProviderParams {
            entry: &e,
            policy: &policy,
            api_key: "test-key".to_owned(),
        })
        .expect("should succeed");
        assert_eq!(provider.name(), "gemini-image");
        assert_eq!(provider.default_model(), "gemini-2.5-flash-image");
    }

    #[test]
    fn create_openai_image_provider_succeeds() {
        let e = entry("openai");
        let policy = AiProviderConfig::default();
        let provider = ImageProviderFactory::create(&ImageProviderParams {
            entry: &e,
            policy: &policy,
            api_key: "test-key".to_owned(),
        })
        .expect("should succeed");
        assert_eq!(provider.name(), "openai-image");
        assert_eq!(provider.default_model(), "gpt-image-1");
    }

    #[test]
    fn create_fails_when_disabled() {
        let e = entry("gemini");
        let policy = AiProviderConfig {
            enabled: false,
            ..AiProviderConfig::default()
        };
        let error = ImageProviderFactory::create(&ImageProviderParams {
            entry: &e,
            policy: &policy,
            api_key: "test-key".to_owned(),
        })
        .err()
        .expect("should be an error");
        assert!(error.to_string().contains("disabled"));
    }

    #[test]
    fn anthropic_protocol_does_not_support_images() {
        let e = entry("anthropic");
        let policy = AiProviderConfig::default();
        let error = ImageProviderFactory::create(&ImageProviderParams {
            entry: &e,
            policy: &policy,
            api_key: "test-key".to_owned(),
        })
        .err()
        .expect("should be an error");
        assert!(
            error
                .to_string()
                .contains("does not support image generation")
        );
    }

    #[test]
    fn gemini_default_image_model_override() {
        let e = entry("gemini");
        let policy = AiProviderConfig {
            default_image_model: "gemini-3-pro-image-preview".to_owned(),
            ..AiProviderConfig::default()
        };
        let provider = ImageProviderFactory::create(&ImageProviderParams {
            entry: &e,
            policy: &policy,
            api_key: "test-key".to_owned(),
        })
        .expect("should succeed");
        assert_eq!(provider.default_model(), "gemini-3-pro-image-preview");
    }
}

mod supports_image_generation_tests {
    use super::*;

    #[test]
    fn image_protocols_supported() {
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::Gemini
        ));
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::OpenAiChat
        ));
        assert!(ImageProviderFactory::supports_image_generation(
            WireProtocol::OpenAiResponses
        ));
    }

    #[test]
    fn anthropic_not_supported() {
        assert!(!ImageProviderFactory::supports_image_generation(
            WireProtocol::Anthropic
        ));
    }
}
