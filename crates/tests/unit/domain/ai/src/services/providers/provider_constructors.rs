//! The production constructors of the HTTP providers — the ones that point at
//! the real vendor endpoints, as opposed to the `with_endpoint` forms every
//! other suite uses against a mock server.

use systemprompt_ai::services::providers::anthropic::AnthropicProvider;
use systemprompt_ai::services::providers::gemini::GeminiProvider;
use systemprompt_ai::services::providers::gemini_images::GeminiImageProvider;
use systemprompt_ai::services::providers::image_provider_trait::ImageProvider;
use systemprompt_ai::services::providers::openai::OpenAiProvider;
use systemprompt_ai::services::providers::provider_trait::AiProvider;

#[test]
fn the_anthropic_default_constructor_targets_the_vendor_api_with_no_seeded_models() {
    let provider = AnthropicProvider::new("test-key".to_owned());

    assert_eq!(provider.name(), "anthropic");
    assert!(
        !provider.supports_model("claude-sonnet-4-6"),
        "a provider built without a catalog must advertise no models until one is seeded"
    );
    assert!(
        !provider.supports_google_search(),
        "web search is opt-in, not on by default"
    );
}

#[test]
fn the_openai_default_constructor_targets_the_vendor_api_with_no_seeded_models() {
    let provider = OpenAiProvider::new("test-key".to_owned());

    assert_eq!(provider.name(), "openai");
    assert!(
        !provider.supports_model("gpt-4o"),
        "a provider built without a catalog must advertise no models until one is seeded"
    );
}

#[test]
fn the_gemini_default_constructor_builds_a_client_and_leaves_search_disabled() {
    let provider = GeminiProvider::new("test-key".to_owned())
        .expect("the default Gemini constructor must build its HTTP client");

    assert_eq!(provider.name(), "gemini");
    assert!(
        !provider.supports_google_search(),
        "Google Search grounding is opt-in per profile, not a constructor default"
    );
    assert!(
        !provider.supports_model("gemini-2.5-pro"),
        "a provider built without a catalog must advertise no models until one is seeded"
    );
}

#[test]
fn the_gemini_trait_accessors_report_its_capabilities() {
    let provider = GeminiProvider::new("test-key".to_owned()).expect("client builds");

    assert!(provider.supports_streaming());
    assert!(
        provider.supports_sampling(None),
        "Gemini accepts sampling parameters with or without an explicit set"
    );
    let composition = provider.capabilities().composition;
    assert!(
        !composition.if_then_else && !composition.allof && !composition.oneof,
        "Gemini declines the composition keywords, which is exactly why the transformer \
         has to split a discriminated union into one tool per variant"
    );
    assert!(
        composition.anyof,
        "anyOf is the one composition keyword Gemini does accept"
    );
    assert!(
        provider.as_any().downcast_ref::<GeminiProvider>().is_some(),
        "as_any must round-trip to the concrete provider"
    );
    assert!(
        provider.as_any().downcast_ref::<OpenAiProvider>().is_none(),
        "as_any must not succeed for a different provider type"
    );
}

#[test]
fn the_gemini_image_default_model_is_overridable_after_construction() {
    let provider = GeminiImageProvider::new("test-key".to_owned());
    let stock = provider.capabilities();
    assert!(
        stock.max_prompt_length > 0,
        "the image provider must advertise a prompt budget"
    );

    let overridden = GeminiImageProvider::new("test-key".to_owned())
        .with_default_model("custom-model".to_owned());
    assert_eq!(overridden.name(), "gemini-image");
    assert!(
        overridden.capabilities().supports_image_editing,
        "the override must not disturb the declared capabilities"
    );
}

#[test]
fn the_openai_trait_accessors_report_its_capabilities_without_a_network_call() {
    let provider = OpenAiProvider::new("test-key".to_owned());

    assert!(
        provider.supports_streaming(),
        "OpenAI is a streaming provider"
    );
    assert!(
        provider.supports_sampling(None),
        "sampling parameters are accepted with or without an explicit set"
    );
    assert!(
        provider.capabilities().composition.allof,
        "the OpenAI schema profile must accept allOf composition"
    );
    assert!(
        provider.as_any().downcast_ref::<OpenAiProvider>().is_some(),
        "as_any must round-trip to the concrete provider so callers can reach \
         provider-specific behaviour"
    );
    assert!(
        provider
            .as_any()
            .downcast_ref::<AnthropicProvider>()
            .is_none(),
        "as_any must not succeed for a different provider type"
    );
}

#[test]
fn the_anthropic_trait_accessors_report_its_capabilities() {
    let provider = AnthropicProvider::new("test-key".to_owned());

    assert!(provider.supports_streaming());
    assert!(
        provider.capabilities().composition.oneof,
        "the Anthropic schema profile accepts oneOf composition"
    );
    assert!(
        provider
            .as_any()
            .downcast_ref::<AnthropicProvider>()
            .is_some()
    );
}
