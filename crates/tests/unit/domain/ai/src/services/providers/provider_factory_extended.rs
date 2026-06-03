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
    wire: WireProtocol,
    endpoint: &str,
    google_search_enabled: bool,
) -> std::sync::Arc<dyn AiProvider> {
    let models = seed_models(name);
    let resilience = ResilienceSettings::default();
    let params = ProviderClientParams {
        name,
        wire,
        endpoint,
        api_key: "test-key".to_owned(),
        google_search_enabled,
        resilience: &resilience,
        models: &models,
        default_model: None,
    };
    ProviderFactory::create(&params, None).expect("factory creates the provider")
}

mod web_search_enablement {
    use super::*;

    #[test]
    fn openai_search_toggle() {
        let on = create(
            "openai",
            WireProtocol::OpenAiChat,
            "https://api.openai.com/v1",
            true,
        );
        assert_eq!(on.name(), "openai");
        assert!(on.supports_google_search());

        let off = create(
            "openai",
            WireProtocol::OpenAiChat,
            "https://api.openai.com/v1",
            false,
        );
        assert!(!off.supports_google_search());
    }

    #[test]
    fn anthropic_search_toggle() {
        let on = create(
            "anthropic",
            WireProtocol::Anthropic,
            "https://api.anthropic.com/v1",
            true,
        );
        assert_eq!(on.name(), "anthropic");
        assert!(on.supports_google_search());

        let off = create(
            "anthropic",
            WireProtocol::Anthropic,
            "https://api.anthropic.com/v1",
            false,
        );
        assert!(!off.supports_google_search());
    }

    #[test]
    fn custom_endpoint_is_accepted() {
        let provider = create(
            "gemini",
            WireProtocol::Gemini,
            "https://custom-gemini.example.com",
            false,
        );
        assert_eq!(provider.name(), "gemini");
    }
}
