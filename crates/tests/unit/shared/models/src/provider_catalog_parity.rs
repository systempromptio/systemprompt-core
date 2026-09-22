//! Regression lock for the embedded default provider catalog.
//!
//! These tests transcribe the support-set and per-million pricing that the
//! deleted hardcoded `AnthropicModels`/`OpenAiModels`/`GeminiModels` tables
//! used to encode. `ProviderRegistry::default_seed()` is now the single source
//! of that knowledge, so an accidental edit to `default_catalog.yaml` (a wrong
//! price, a dropped model, a renamed default) must fail here.

use systemprompt_models::services::ProviderRegistry;

fn seed() -> ProviderRegistry {
    ProviderRegistry::default_seed().expect("embedded default catalog parses")
}

fn pricing(registry: &ProviderRegistry, provider: &str, model: &str) -> (f64, f64) {
    let entry = registry
        .find_provider(provider)
        .unwrap_or_else(|| panic!("provider '{provider}' present in seed"));
    let m = entry
        .find_model(model)
        .unwrap_or_else(|| panic!("model '{model}' present under '{provider}'"));
    (m.pricing.input_per_million, m.pricing.output_per_million)
}

fn assert_pricing(
    registry: &ProviderRegistry,
    provider: &str,
    model: &str,
    input: f64,
    output: f64,
) {
    let (got_in, got_out) = pricing(registry, provider, model);
    assert!(
        (got_in - input).abs() < f64::EPSILON,
        "{provider}/{model} input price drifted: expected {input}, got {got_in}"
    );
    assert!(
        (got_out - output).abs() < f64::EPSILON,
        "{provider}/{model} output price drifted: expected {output}, got {got_out}"
    );
}

#[test]
fn seed_parses_and_validates() {
    let registry = seed();
    registry
        .validate()
        .expect("seed catalog passes registry validation (unique names/models, sane endpoints)");
}

#[test]
fn anthropic_pricing_baseline() {
    let registry = seed();
    assert_pricing(&registry, "anthropic", "claude-sonnet-5", 2.0, 10.0);
    assert_pricing(&registry, "anthropic", "claude-opus-5-5", 4.0, 20.0);
    assert_pricing(&registry, "anthropic", "claude-opus-5", 5.0, 25.0);
    assert_pricing(
        &registry,
        "anthropic",
        "claude-haiku-4-5-20251001",
        1.0,
        5.0,
    );
}

#[test]
fn anthropic_advertises_current_lineup_and_keeps_active_legacy_routable() {
    let registry = seed();
    assert_eq!(
        registry.advertised_model_ids(&[systemprompt_models::services::ApiSurface::Anthropic]),
        vec![
            "claude-sonnet-5".to_owned(),
            "claude-opus-5-5".to_owned(),
            "claude-opus-5".to_owned(),
            "claude-fable-5-1".to_owned(),
            "claude-haiku-4-5-20251001".to_owned(),
        ]
    );

    assert!(
        registry.contains_model("claude-fable-5"),
        "claude-fable-5 is hidden but must remain routable"
    );

    for retired in [
        "claude-opus-4-1-20250805",
        "claude-sonnet-4-20250514",
        "claude-opus-4-20250514",
        "claude-sonnet-4-6",
        "claude-opus-4-8",
        "claude-opus-4-7",
        "claude-opus-4-6",
        "claude-opus-4-5-20251101",
        "claude-sonnet-4-5-20250929",
    ] {
        assert!(!registry.contains_model(retired), "{retired} is retired");
    }
}

#[test]
fn openai_pricing_baseline() {
    let registry = seed();
    assert_pricing(&registry, "openai", "gpt-4.1", 2.0, 8.0);
    assert_pricing(&registry, "openai", "gpt-4.1-mini", 0.4, 1.6);
    assert_pricing(&registry, "openai", "gpt-5", 1.25, 10.0);
    assert_pricing(&registry, "openai", "gpt-5-mini", 0.25, 2.0);
    assert_pricing(&registry, "openai", "gpt-4o", 2.5, 10.0);
    assert_pricing(&registry, "openai", "gpt-4o-mini", 0.15, 0.6);
    assert_pricing(&registry, "openai", "gpt-4-turbo", 10.0, 30.0);
    assert_pricing(&registry, "openai", "gpt-3.5-turbo", 0.5, 1.5);
    assert_pricing(&registry, "openai", "o1", 15.0, 60.0);
    assert_pricing(&registry, "openai", "o3", 10.0, 40.0);
    assert_pricing(&registry, "openai", "o3-mini", 1.1, 4.4);
    assert_pricing(&registry, "openai", "o4-mini", 1.1, 4.4);
}

#[test]
fn gemini_pricing_baseline() {
    let registry = seed();
    assert_pricing(&registry, "gemini", "gemini-2.5-pro", 1.25, 10.0);
    assert_pricing(&registry, "gemini", "gemini-2.5-flash", 0.3, 2.5);
    assert_pricing(&registry, "gemini", "gemini-2.0-flash", 0.1, 0.4);
    assert_pricing(&registry, "gemini", "gemini-2.0-flash-lite", 0.1, 0.4);
    assert_pricing(&registry, "gemini", "gemini-3.1-pro-preview", 2.0, 12.0);
}

#[test]
fn provider_default_models_are_stable() {
    let registry = seed();
    for (provider, expected_default) in [
        ("anthropic", "claude-sonnet-5"),
        ("openai", "gpt-4.1"),
        ("gemini", "gemini-3.1-flash-lite-preview"),
    ] {
        let first = registry
            .find_provider(provider)
            .and_then(|e| e.models.first())
            .map(|m| m.id.as_str())
            .unwrap_or_else(|| panic!("provider '{provider}' has at least one model"));
        assert_eq!(
            first, expected_default,
            "agent-side default for '{provider}' is the first seeded model"
        );
    }
}

#[test]
fn image_models_are_present_with_per_image_pricing() {
    let registry = seed();
    for (provider, model) in [
        ("openai", "gpt-image-1"),
        ("openai", "gpt-image-1-mini"),
        ("gemini", "gemini-2.5-flash-image"),
        ("gemini", "gemini-3.1-flash-image-preview"),
    ] {
        let m = registry
            .find_provider(provider)
            .and_then(|e| e.find_model(model))
            .unwrap_or_else(|| panic!("image model '{model}' present under '{provider}'"));
        assert!(
            m.capabilities.image_generation,
            "{provider}/{model} declares image_generation"
        );
        assert_eq!(
            m.pricing.per_image_cents,
            Some(4.0),
            "{provider}/{model} carries per-image pricing"
        );
    }
}

#[test]
fn unknown_model_is_not_in_seed() {
    let registry = seed();
    assert!(!registry.contains_model("definitely-not-a-real-model"));
}

/// The models in day-to-day use must be in the seed, or a freshly generated
/// profile prices them at zero — the exact gap that let a whole run of gateway
/// traffic audit as free.
#[test]
fn anthropic_seed_covers_the_current_generation() {
    let registry = seed();
    assert_pricing(&registry, "anthropic", "claude-opus-5-5", 4.0, 20.0);
    assert_pricing(&registry, "anthropic", "claude-opus-5", 5.0, 25.0);
    assert_pricing(&registry, "anthropic", "claude-sonnet-5", 2.0, 10.0);
    assert_pricing(&registry, "anthropic", "claude-fable-5-1", 10.0, 50.0);
    assert_pricing(&registry, "anthropic", "claude-fable-5", 10.0, 50.0);
}

#[test]
fn anthropic_seed_declares_prompt_caching_support() {
    let registry = seed();
    let anthropic = registry
        .find_provider("anthropic")
        .expect("Anthropic provider is seeded");

    assert!(
        anthropic
            .models
            .iter()
            .all(|model| model.capabilities.prompt_caching),
        "every active seeded Claude model supports prompt caching"
    );
    assert!(
        !registry
            .find_provider("openai")
            .expect("OpenAI provider is seeded")
            .find_model("gpt-5")
            .expect("GPT-5 is seeded")
            .capabilities
            .prompt_caching,
        "automatic Anthropic cache control is not inferred from cache pricing"
    );
}

/// Dated ids are still sent by pinned clients, and matching is
/// exact-id-or-alias with no glob, so a dropped alias prices at zero.
#[test]
fn anthropic_dated_ids_resolve_through_aliases() {
    let registry = seed();
    assert_pricing(
        &registry,
        "anthropic",
        "claude-haiku-4-5-20251001",
        1.0,
        5.0,
    );
    assert_pricing(&registry, "anthropic", "claude-haiku-4-5", 1.0, 5.0);
}

#[test]
fn anthropic_seed_prices_cache_tokens() {
    let registry = seed();
    let entry = registry.find_provider("anthropic").expect("anthropic seed");
    for model in &entry.models {
        let p = model.pricing;
        let (read, write) = (p.cache_read_rate(), p.cache_write_rate());
        assert!(
            p.declares_cache_rate() && read > 0.0 && write > 0.0,
            "{} has no cache rates: a cache-heavy agent loop would bill as free",
            model.id.as_str()
        );
        assert!(
            (read - p.input_per_million * 0.1).abs() < 1e-9,
            "{} cache read is not 0.1x input",
            model.id.as_str()
        );
        assert!(
            (write - p.input_per_million * 1.25).abs() < 1e-9,
            "{} cache write is not 1.25x input",
            model.id.as_str()
        );
    }
}
