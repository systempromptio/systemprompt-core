use std::collections::HashSet;

use chrono::NaiveDate;
use systemprompt_catalog_fixtures::{CatalogFixture, model, platform};
use systemprompt_loader::vertex_discovery::merge;
use systemprompt_loader::vertex_discovery::source::DiscoveryEndpoint;
use systemprompt_manifest::services::catalog::{
    CatalogDocument, EndpointConfig, EndpointRef, ModelEntry, ProviderConfig,
};
use systemprompt_manifest::services::{
    DiscoveryReport, ModelPricing, VertexRateCard, VertexRateCardEntry,
};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

const MAAS_URL: &str = "https://aiplatform.googleapis.com/v1beta1/projects/{project}/locations/global/endpoints/openapi";

fn provider() -> CatalogDocument {
    let declared = ModelEntry {
        upstream_model: Some("openai/gpt-oss-20b-maas".to_owned()),
        ..model(
            &[ClientFamily::OpenAi],
            0,
            ModelPricing {
                input_per_million: 0.07,
                output_per_million: 0.3,
                cache_read_per_million: Some(0.0),
                ..ModelPricing::default()
            },
        )
    };
    CatalogFixture::new()
        .credential("vertex", platform("vertex_maas"))
        .provider("vertex", ProviderConfig::new("vertex"))
        .endpoint("vertex", "maas", EndpointConfig::new(WireProtocol::OpenAiChat, MAAS_URL))
        .model("openai.gpt-oss-20b", declared)
        .deploy("openai.gpt-oss-20b", ["vertex/maas"])
        .document()
        .clone()
}

fn endpoint() -> DiscoveryEndpoint {
    DiscoveryEndpoint {
        endpoint: EndpointRef::new("vertex", "maas"),
        wire: WireProtocol::OpenAiChat,
        url: MAAS_URL.to_owned(),
    }
}

fn card() -> VertexRateCard {
    VertexRateCard::embedded().expect("the embedded rate card parses")
}

// Why a fixed date: every lifecycle decision is a function of the calendar,
// and a test that read the clock would change its answer on 2026-09-21.
fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 11).expect("a valid date")
}

fn entry(card: &VertexRateCard, upstream: &str) -> VertexRateCardEntry {
    card.lookup(upstream)
        .unwrap_or_else(|| panic!("{upstream} is priced"))
        .clone()
}

// Why: the catalog is hand-written and reviewed; discovery is a convenience.
// A convenience that rewrites a reviewed price is worse than one that adds
// nothing, so an explicit declaration always wins.
#[test]
fn an_explicitly_declared_id_wins_and_the_registry_is_untouched() {
    let card = card();
    let mut provider = provider();
    let mut report = DiscoveryReport::default();

    merge::publish(
        &mut provider,
        &endpoint(),
        &entry(&card, "openai/gpt-oss-20b-maas"),
        today(),
        &mut report,
    );

    assert_eq!(report.explicit_wins, vec!["openai.gpt-oss-20b".to_string()]);
    assert!(report.discovered_priced.is_empty());
    assert_eq!(provider.models.models.len(), 1);
    let declared = provider.models.get("openai.gpt-oss-20b").expect("declared");
    assert!(
        (declared.pricing.input_per_million - 0.07).abs() < f64::EPSILON,
        "the declared price survived"
    );
}

#[test]
fn a_priced_model_the_catalog_does_not_declare_is_appended() {
    let card = card();
    let mut provider = provider();
    let mut report = DiscoveryReport::default();

    merge::publish(
        &mut provider,
        &endpoint(),
        &entry(&card, "qwen/qwen3-235b-a22b-instruct-2507-maas"),
        today(),
        &mut report,
    );

    assert_eq!(
        report.discovered_priced,
        vec!["qwen.qwen3-235b".to_string()]
    );
    assert!(report.explicit_wins.is_empty());
    let added = provider
        .models
        .get("qwen.qwen3-235b")
        .expect("the discovered model is now servable");
    assert_eq!(
        provider.deployments.get("qwen.qwen3-235b").map(|t| t[0].endpoint().to_string()),
        Some("vertex/maas".to_owned()),
        "it is deployed on the endpoint that listed it"
    );
    assert_eq!(added.clients, vec![ClientFamily::OpenAi], "advertised like its endpoint");
    assert_eq!(
        added.upstream_model.as_deref(),
        Some("qwen/qwen3-235b-a22b-instruct-2507-maas")
    );
    assert!(added.pricing.is_billable());
}

#[test]
fn an_unpriced_model_is_reported_by_its_vertex_name_and_never_registered() {
    let provider = provider();
    let mut report = DiscoveryReport::default();

    merge::record_unpriced("qwen/qwen3-embedding-8b".to_string(), &mut report);
    merge::record_unpriced("qwen/qwen3-embedding-8b".to_string(), &mut report);

    assert_eq!(
        report.discovered_unpriced,
        vec!["qwen/qwen3-embedding-8b".to_string()],
        "a repeat across pages is still one finding"
    );
    assert!(provider.models.get("qwen/qwen3-embedding-8b").is_none());
    assert_eq!(provider.models.models.len(), 1);
}

// Why: a listing gap is Google's editorial decision, not our deprecation --
// gpt-oss-20b-maas is priced, routed, and absent from the published catalog.
#[test]
fn a_priced_model_no_listing_returned_is_reported_with_its_declaration_intact() {
    let card = card();
    let provider = provider();
    let mut report = DiscoveryReport::default();
    let mut seen: HashSet<String> = HashSet::new();
    for entry in card.entries_for(WireProtocol::OpenAiChat) {
        if entry.upstream != "openai/gpt-oss-20b-maas" {
            seen.insert(entry.upstream.clone());
        }
    }

    merge::record_unseen(&card, WireProtocol::OpenAiChat, &seen, &mut report);

    assert_eq!(
        report.priced_not_published,
        vec!["openai.gpt-oss-20b".to_string()]
    );
    assert!(
        provider.models.get("openai.gpt-oss-20b").is_some(),
        "discovery never removes an explicit declaration"
    );
}

#[test]
fn a_provider_whose_card_entries_all_listed_reports_no_gap() {
    let card = card();
    let seen: HashSet<String> = card
        .entries_for(WireProtocol::Gemini)
        .map(|entry| entry.upstream.clone())
        .collect();
    let mut report = DiscoveryReport::default();

    merge::record_unseen(&card, WireProtocol::Gemini, &seen, &mut report);

    assert!(report.priced_not_published.is_empty());
}

// Why: Vertex keeps listing a model right up to its retirement date, so the
// documentation's date is the only thing that stops a retiring model being
// published to a developer who would lose it mid-project.
#[test]
fn a_model_inside_its_retirement_window_is_withheld_and_reported() {
    let card = card();
    let mut provider = provider();
    let mut report = DiscoveryReport::default();
    let after_notice = NaiveDate::from_ymd_opt(2026, 9, 25).expect("a valid date");

    merge::publish(
        &mut provider,
        &endpoint(),
        &entry(&card, "qwen/qwen3-235b-a22b-instruct-2507-maas"),
        after_notice,
        &mut report,
    );

    assert_eq!(report.retiring, vec!["qwen.qwen3-235b".to_string()]);
    assert!(report.discovered_priced.is_empty());
    assert!(provider.models.get("qwen.qwen3-235b").is_none());
}

// Why: the catalog is the operator's; a retiring model they declared by hand
// keeps being served, and the report says so instead of silently agreeing.
#[test]
fn an_explicit_declaration_of_a_retiring_model_is_kept_but_reported_as_retiring() {
    let card = card();
    let mut provider = provider();
    let mut report = DiscoveryReport::default();
    let after_notice = NaiveDate::from_ymd_opt(2026, 10, 1).expect("a valid date");

    merge::publish(
        &mut provider,
        &endpoint(),
        &entry(&card, "openai/gpt-oss-20b-maas"),
        after_notice,
        &mut report,
    );

    assert_eq!(report.retiring, vec!["openai.gpt-oss-20b".to_string()]);
    assert!(report.explicit_wins.is_empty());
    assert!(provider.models.get("openai.gpt-oss-20b").is_some());
}

// Why: removing a retired declaration empties the route that reached it, and
// a route reaching no priced model fails boot; the date passing must not stop
// the instance. The model stays priced and is no longer advertised instead.
#[test]
fn an_explicit_declaration_past_its_retirement_date_is_unadvertised_not_removed() {
    let card = card();
    let mut provider = provider();
    let mut report = DiscoveryReport::default();
    let after_retirement = NaiveDate::from_ymd_opt(2026, 10, 22).expect("a valid date");

    merge::publish(
        &mut provider,
        &endpoint(),
        &entry(&card, "openai/gpt-oss-20b-maas"),
        after_retirement,
        &mut report,
    );

    let model = provider
        .models
        .get("openai.gpt-oss-20b")
        .expect("a retired declaration stays in the catalog");
    assert!(model.clients.is_empty(), "a retired declaration leaves every listing");
    assert!(model.pricing.is_billable());
    assert_eq!(report.retiring, vec!["openai.gpt-oss-20b".to_string()]);
}
