use systemprompt_ai::services::providers::{AiProvider, ProviderClientParams, ProviderFactory};
use systemprompt_models::profile::{ProviderModel, ProviderRegistry, WireProtocol};
use systemprompt_models::services::ResilienceSettings;

fn seed_models(provider: &str) -> Vec<ProviderModel> {
    ProviderRegistry::default_seed()
        .expect("embedded default catalog parses")
        .find_provider(provider)
        .expect("provider present in default catalog")
        .models
        .clone()
}

fn create(
    name: &str,
    protocol: WireProtocol,
    endpoint: &str,
    google_search_enabled: bool,
) -> std::sync::Arc<dyn AiProvider> {
    let models = seed_models(name);
    let resilience = ResilienceSettings::default();
    let params = ProviderClientParams {
        name,
        protocol,
        endpoint,
        api_key: "test-key".to_owned(),
        google_search_enabled,
        resilience: &resilience,
        models: &models,
        default_model: None,
    };
    ProviderFactory::create(&params, None).expect("factory creates the provider")
}

mod create_tests {
    use super::*;

    #[test]
    fn create_anthropic_resolves_catalog() {
        let provider = create(
            "anthropic",
            WireProtocol::Anthropic,
            "https://api.anthropic.com/v1",
            false,
        );
        assert_eq!(provider.name(), "anthropic");
        assert!(provider.supports_model("claude-sonnet-4-6"));
        assert_eq!(provider.default_model(), "claude-sonnet-4-6");
        assert!(!provider.supports_model("gpt-4.1"));
    }

    #[test]
    fn create_openai_resolves_catalog() {
        let provider = create(
            "openai",
            WireProtocol::OpenAiChat,
            "https://api.openai.com/v1",
            false,
        );
        assert_eq!(provider.name(), "openai");
        assert!(provider.supports_model("gpt-4.1"));
        assert_eq!(provider.default_model(), "gpt-4.1");
    }

    #[test]
    fn create_gemini_resolves_catalog() {
        let provider = create(
            "gemini",
            WireProtocol::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
            false,
        );
        assert_eq!(provider.name(), "gemini");
        assert!(provider.supports_model("gemini-2.5-flash"));
        assert_eq!(provider.default_model(), "gemini-3.1-flash-lite-preview");
    }

    #[test]
    fn gemini_google_search_toggle() {
        let enabled = create(
            "gemini",
            WireProtocol::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
            true,
        );
        assert!(enabled.supports_google_search());

        let disabled = create(
            "gemini",
            WireProtocol::Gemini,
            "https://generativelanguage.googleapis.com/v1beta",
            false,
        );
        assert!(!disabled.supports_google_search());
    }

    #[test]
    fn pricing_comes_from_the_catalog() {
        let provider = create(
            "anthropic",
            WireProtocol::Anthropic,
            "https://api.anthropic.com/v1",
            false,
        );
        let pricing = provider.get_pricing("claude-haiku-4-5-20251001");
        assert!((pricing.input_per_million - 1.0).abs() < f64::EPSILON);
        assert!((pricing.output_per_million - 5.0).abs() < f64::EPSILON);
    }
}
