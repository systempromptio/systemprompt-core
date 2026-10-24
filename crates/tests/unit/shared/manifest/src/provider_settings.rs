//! The homes of the settings `ai.providers` used to carry: provider and
//! endpoint `resilience`, provider `google_search_enabled`, and
//! `ai.default_image_model`.

use systemprompt_catalog_fixtures::{
    claude_model, model, one_platform_provider, priced, vertex_regions,
};
use systemprompt_manifest::services::catalog::{
    Capability, CatalogError, EndpointConfig, ProviderConfig,
};
use systemprompt_manifest::services::{AiConfig, ResilienceSettings};
use systemprompt_models::providers::ClientFamily;

#[test]
fn an_endpoint_resilience_override_wins_over_the_provider_default() {
    let provider_default = ResilienceSettings {
        retry_attempts: 7,
        ..ResilienceSettings::default()
    };
    let eu_override = ResilienceSettings {
        retry_attempts: 1,
        request_timeout_ms: 5_000,
        ..ResilienceSettings::default()
    };
    let catalog = vertex_regions()
        .with_provider("vertex", |p| {
            p.resilience = provider_default;
            if let Some(eu) = p.endpoints.get_mut("claude-eu") {
                eu.resilience = Some(eu_override);
            }
        })
        .build();
    let hops = catalog.deployments("claude-sonnet-5");
    assert_eq!(hops[0].resilience, provider_default);
    assert_eq!(hops[1].resilience, eu_override);
    assert_eq!(
        catalog.provider("vertex").expect("declared").resilience,
        provider_default
    );
}

#[test]
fn resilience_parses_at_both_levels() {
    let yaml = "credential: k\nresilience: { retry_attempts: 2 }\nendpoints:\n  api: { wire: anthropic, url: \"https://api.anthropic.com/v1\", resilience: { request_timeout_ms: 9000 } }\n";
    let provider: ProviderConfig = serde_yaml::from_str(yaml).expect("parses");
    assert_eq!(provider.resilience.retry_attempts, 2);
    let endpoint: &EndpointConfig = provider.endpoint("api").expect("endpoint");
    assert_eq!(
        endpoint.resilience.map(|r| r.request_timeout_ms),
        Some(9_000)
    );
    assert!(!provider.google_search_enabled);
}

#[test]
fn google_search_needs_a_gemini_endpoint() {
    let catalog = vertex_regions()
        .with_provider("vertex", |p| p.google_search_enabled = true)
        .build();
    let provider = catalog.provider("vertex").expect("declared");
    assert!(provider.google_search_enabled);
    assert!(catalog.deployments("vertex-gemini-3.8-flash")[0].google_search_enabled);

    let error = one_platform_provider()
        .with_provider("anthropic", |p| p.google_search_enabled = true)
        .try_build()
        .expect_err("refused");
    assert!(
        matches!(error, CatalogError::GoogleSearchWithoutGemini { .. }),
        "{error}"
    );
}

#[test]
fn the_default_image_model_is_a_deployed_image_model() {
    let mut image = model(&[ClientFamily::OpenAi], 0, priced(0.0, 0.0, 0.0));
    image.pricing.per_image_cents = Some(4.0);
    image.capabilities.insert(Capability::ImageGeneration, true);
    let base = one_platform_provider()
        .model("image-1", image)
        .deploy("image-1", ["anthropic/api"]);
    let catalog = base.clone().default_image_model("image-1").build();
    assert_eq!(
        catalog.default_image_model().map(|m| m.as_str()),
        Some("image-1")
    );

    let unknown = base
        .clone()
        .default_image_model("ghost")
        .try_build()
        .expect_err("refused");
    assert!(matches!(
        unknown,
        CatalogError::UnknownDefaultImageModel { .. }
    ));

    let text_only = one_platform_provider()
        .model("text", claude_model(1000))
        .deploy("text", ["anthropic/api"])
        .default_image_model("text")
        .try_build()
        .expect_err("refused");
    assert!(matches!(
        text_only,
        CatalogError::DefaultImageModelNotImageCapable { .. }
    ));
}

#[test]
fn ai_default_image_model_parses() {
    let ai: AiConfig = serde_yaml::from_str("default_image_model: gpt-image-1\n").expect("parses");
    assert_eq!(
        ai.default_image_model
            .map(|m| m.as_str().to_owned())
            .as_deref(),
        Some("gpt-image-1")
    );
}
