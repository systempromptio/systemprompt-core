use std::collections::HashSet;

use chrono::NaiveDate;
use systemprompt_loader::vertex_discovery::merge;
use systemprompt_models::services::{
    DiscoveryReport, ProviderEntry, VertexRateCard, VertexRateCardEntry,
};

const MAAS_PROVIDER: &str = r#"
name: vertex-maas
wire: openai-chat
surface: openai
endpoint: https://aiplatform.googleapis.com/v1beta1/projects/{project}/locations/global/endpoints/openapi
api_key_secret: vertex_maas
models:
- id: openai.gpt-oss-20b
  upstream_model: openai/gpt-oss-20b-maas
  pricing:
    input_per_million: 0.07
    output_per_million: 0.3
"#;

fn provider() -> ProviderEntry {
    serde_yaml::from_str(MAAS_PROVIDER).expect("the fixture provider parses")
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
        &entry(&card, "openai/gpt-oss-20b-maas"),
        today(),
        &mut report,
    );

    assert_eq!(report.explicit_wins, vec!["openai.gpt-oss-20b".to_string()]);
    assert!(report.discovered_priced.is_empty());
    assert_eq!(provider.models.len(), 1);
    let declared = &provider.models[0];
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
        .find_model("qwen.qwen3-235b")
        .expect("the discovered model is now servable");
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
    assert!(provider.find_model("qwen/qwen3-embedding-8b").is_none());
    assert_eq!(provider.models.len(), 1);
}

// Why: a listing gap is Google's editorial decision, not our deprecation --
// gpt-oss-20b-maas is priced, routed, and absent from the published catalog.
#[test]
fn a_priced_model_no_listing_returned_is_reported_with_its_declaration_intact() {
    let card = card();
    let provider = provider();
    let mut report = DiscoveryReport::default();
    let mut seen: HashSet<String> = HashSet::new();
    for entry in card.entries_for("vertex-maas") {
        if entry.upstream != "openai/gpt-oss-20b-maas" {
            seen.insert(entry.upstream.clone());
        }
    }

    merge::record_unseen(&card, "vertex-maas", &seen, &mut report);

    assert_eq!(
        report.priced_not_published,
        vec!["openai.gpt-oss-20b".to_string()]
    );
    assert!(
        provider.find_model("openai.gpt-oss-20b").is_some(),
        "discovery never removes an explicit declaration"
    );
}

#[test]
fn a_provider_whose_card_entries_all_listed_reports_no_gap() {
    let card = card();
    let seen: HashSet<String> = card
        .entries_for("vertex")
        .map(|entry| entry.upstream.clone())
        .collect();
    let mut report = DiscoveryReport::default();

    merge::record_unseen(&card, "vertex", &seen, &mut report);

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
        &entry(&card, "qwen/qwen3-235b-a22b-instruct-2507-maas"),
        after_notice,
        &mut report,
    );

    assert_eq!(report.retiring, vec!["qwen.qwen3-235b".to_string()]);
    assert!(report.discovered_priced.is_empty());
    assert!(provider.find_model("qwen.qwen3-235b").is_none());
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
        &entry(&card, "openai/gpt-oss-20b-maas"),
        after_notice,
        &mut report,
    );

    assert_eq!(report.retiring, vec!["openai.gpt-oss-20b".to_string()]);
    assert!(report.explicit_wins.is_empty());
    assert!(provider.find_model("openai.gpt-oss-20b").is_some());
}
