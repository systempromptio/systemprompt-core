//! Lock for core's embedded default catalog (`DefaultCatalog`): it resolves,
//! its prices and line-up are the reconciled ones, and its one alias is a
//! route, not a second model.

use systemprompt_manifest::services::catalog::{
    CatalogContext, DefaultCatalog, ModelRequest, ResolvedCatalog,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

fn catalog() -> ResolvedCatalog {
    DefaultCatalog::document()
        .expect("embedded default catalog parses")
        .resolve(&CatalogContext::default())
        .expect("embedded default catalog resolves")
}

fn assert_price(catalog: &ResolvedCatalog, model: &str, input: f64, output: f64) {
    let hop = &catalog.deployments(model)[0];
    assert!(
        (hop.pricing.input_per_million - input).abs() < f64::EPSILON
            && (hop.pricing.output_per_million - output).abs() < f64::EPSILON,
        "{model} price drifted: {:?}",
        hop.pricing
    );
}

#[test]
fn the_default_catalog_ships_the_five_convention_files() {
    let names: Vec<&str> = DefaultCatalog::FILES
        .iter()
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        names,
        systemprompt_manifest::services::catalog::CATALOG_FILES.to_vec()
    );
    assert!(
        !catalog().gateway_enabled(),
        "the install switches the gateway on"
    );
}

#[test]
fn every_vendor_is_one_provider_with_one_platform_credential() {
    let catalog = catalog();
    for (provider, wire) in [
        ("anthropic", WireProtocol::Anthropic),
        ("openai", WireProtocol::OpenAiChat),
        ("gemini", WireProtocol::Gemini),
    ] {
        let resolved = catalog.provider(provider).expect("declared");
        let credential = catalog
            .credential(resolved.credential.as_str())
            .expect("credential");
        assert_eq!(
            credential.platform_secret().map(|s| s.as_str()),
            Some(provider)
        );
        let endpoint = catalog
            .endpoints()
            .find(|e| e.provider.as_str() == provider)
            .expect("one endpoint");
        assert_eq!(endpoint.wire, wire);
    }
}

#[test]
fn reconciled_prices_hold() {
    let catalog = catalog();
    assert_price(&catalog, "claude-sonnet-5", 2.0, 10.0);
    assert_price(&catalog, "claude-opus-5-5", 4.0, 20.0);
    assert_price(&catalog, "claude-haiku-4-5-20251001", 1.0, 5.0);
    assert_price(&catalog, "gpt-4.1", 2.0, 8.0);
    assert_price(&catalog, "o3", 10.0, 40.0);
    assert_price(&catalog, "gemini-2.5-pro", 1.25, 10.0);
    assert_price(&catalog, "gemini-3.1-pro-preview", 2.0, 12.0);
    for model in catalog.models() {
        let hop = &catalog.deployments(model.id.as_str())[0];
        let token_priced = hop.pricing.input_per_million > 0.0;
        assert!(
            !token_priced || hop.pricing.declares_cache_rate(),
            "{} declares no cache-read rate",
            model.id.as_str()
        );
    }
}

#[test]
fn claude_clients_see_the_current_lineup_and_hidden_models_stay_routable() {
    let catalog = catalog();
    let claude: Vec<&str> = catalog
        .advertised_models(Some(ClientFamily::Claude))
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        claude,
        vec![
            "claude-fable-5-1",
            "claude-haiku-4-5-20251001",
            "claude-opus-5",
            "claude-opus-5-5",
            "claude-sonnet-5",
        ]
    );
    assert!(
        !catalog
            .model("claude-fable-5")
            .expect("declared")
            .is_advertised()
    );
    assert!(
        catalog
            .resolve(&ModelRequest::new("claude-fable-5"))
            .is_ok()
    );
    for retired in ["claude-sonnet-4-5-20250929", "claude-opus-4-1-20250805"] {
        assert!(catalog.model(retired).is_none(), "{retired} is retired");
    }
}

#[test]
fn the_rolling_haiku_name_is_an_alias_route() {
    let catalog = catalog();
    assert!(catalog.model("claude-haiku-4-5").is_none());
    let target = catalog
        .resolve(&ModelRequest::new("claude-haiku-4-5"))
        .expect("alias resolves");
    assert_eq!(target.model.id.as_str(), "claude-haiku-4-5-20251001");
    assert_eq!(target.chain[0].upstream_model, "claude-haiku-4-5-20251001");
}

#[test]
fn image_models_are_priced_per_image_and_capable() {
    let catalog = catalog();
    for id in ["gpt-image-1", "gemini-2.5-flash-image"] {
        let model = catalog.model(id).expect("declared");
        assert!(model.capabilities.image_generation, "{id}");
        assert!(
            model.pricing.per_image_cents.is_some_and(|c| c > 0.0),
            "{id}"
        );
    }
}
