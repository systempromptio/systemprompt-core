use systemprompt_models::services::VertexRateCard;

fn card() -> VertexRateCard {
    VertexRateCard::embedded().expect("the embedded Vertex rate card must parse")
}

#[test]
fn the_embedded_card_names_every_upstream_as_publisher_slash_model() {
    for entry in &card().entries {
        let mut parts = entry.upstream.split('/');
        let publisher = parts.next().unwrap_or_default();
        let model = parts.next().unwrap_or_default();
        assert!(!publisher.is_empty(), "{} has no publisher", entry.upstream);
        assert!(!model.is_empty(), "{} has no model name", entry.upstream);
        assert_eq!(parts.next(), None, "{} has extra segments", entry.upstream);
        assert_eq!(entry.publisher(), publisher);
    }
}

#[test]
fn ids_and_upstreams_are_each_declared_once() {
    let card = card();
    let mut ids: Vec<&str> = card.entries.iter().map(|e| e.id.as_str()).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), unique, "a model id is priced twice");

    let mut upstreams: Vec<&str> = card.entries.iter().map(|e| e.upstream.as_str()).collect();
    upstreams.sort_unstable();
    let unique = upstreams.len();
    upstreams.dedup();
    assert_eq!(upstreams.len(), unique, "an upstream is priced twice");
}

// Why: the card is the gateway's allowlist as well as its price list, and the
// gateway refuses to dispatch to a model it cannot price. An entry that is not
// billable would therefore publish a model that 500s on first use.
#[test]
fn every_entry_is_billable_and_belongs_to_a_vertex_provider() {
    for entry in &card().entries {
        assert!(
            entry.pricing.is_billable(),
            "{} is published but not billable",
            entry.id.as_str()
        );
        assert!(
            matches!(entry.provider.as_str(), "vertex" | "vertex-maas"),
            "{} names provider {}, which is not a Vertex provider",
            entry.id.as_str(),
            entry.provider.as_str()
        );
    }
}

#[test]
fn lookup_finds_an_entry_by_its_vertex_name() {
    let card = card();
    let entry = card
        .lookup("qwen/qwen3-235b-a22b-instruct-2507-maas")
        .expect("the qwen 235b entry is priced");
    assert_eq!(entry.id.as_str(), "qwen.qwen3-235b");
    assert_eq!(entry.provider.as_str(), "vertex-maas");
    assert!(card.lookup("google/gemini-embedding-001").is_none());
}

#[test]
fn publishers_are_listed_once_per_provider() {
    let card = card();
    assert_eq!(card.publishers_for("vertex"), vec!["google".to_string()]);
    let maas = card.publishers_for("vertex-maas");
    assert!(maas.contains(&"qwen".to_string()), "{maas:?}");
    assert!(maas.contains(&"zai-org".to_string()), "{maas:?}");
    assert!(!maas.contains(&"google".to_string()), "{maas:?}");
    let mut sorted = maas.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), maas.len(), "a publisher is listed twice");
}

// Why: discovery publishes `to_provider_model()` verbatim, so anything it
// drops is a model that reaches the gateway mispriced or misrouted.
#[test]
fn to_provider_model_carries_the_id_upstream_name_and_price() {
    let card = card();
    let entry = card
        .lookup("google/gemini-2.5-flash")
        .expect("gemini-2.5-flash is priced");
    let model = entry.to_provider_model();
    assert_eq!(model.id.as_str(), "vertex-gemini-2.5-flash");
    assert_eq!(model.upstream_model.as_deref(), Some("gemini-2.5-flash"));
    assert_eq!(
        model.effective_upstream_model("anything"),
        "gemini-2.5-flash"
    );
    assert!(
        (model.pricing.input_per_million - entry.pricing.input_per_million).abs() < f64::EPSILON
    );
    assert!(
        (model.pricing.output_per_million - entry.pricing.output_per_million).abs() < f64::EPSILON
    );
    assert_eq!(model.limits.context_window, entry.limits.context_window);
    assert!(model.capabilities.tools);
    assert!(
        model.governance.is_none(),
        "a discovered model inherits its provider's posture"
    );
}

// Why: gpt-oss-20b is priced but absent from Google's published listing today.
// It is the canonical priced-not-published case and the reason discovery
// reports a gap instead of deleting the entry.
#[test]
fn a_priced_but_unlisted_model_stays_on_the_card() {
    assert!(card().lookup("openai/gpt-oss-20b-maas").is_some());
}

#[test]
fn only_deliberate_entries_opt_into_preview_launch_stages() {
    for entry in &card().entries {
        if entry.allow_preview {
            assert_eq!(entry.id.as_str(), "zai.glm-5", "unexpected preview opt-in");
        }
    }
}
