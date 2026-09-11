use chrono::NaiveDate;
use systemprompt_models::services::{DocumentedLaunchStage, VertexRateCard};

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
    const OPTED_IN: [&str; 3] = ["zai.glm-5", "zai.glm-5.2", "vertex-gemini-3.1-pro-preview"];
    for entry in &card().entries {
        if entry.allow_preview {
            assert!(
                OPTED_IN.contains(&entry.id.as_str()),
                "unexpected preview opt-in: {}",
                entry.id.as_str()
            );
        }
        if entry.launch_stage == DocumentedLaunchStage::Preview {
            assert!(
                entry.allow_preview,
                "{} is documented as preview without an explicit opt-in",
                entry.id.as_str()
            );
        }
    }
}

// Why: every lifecycle fact on the card is a claim about Google's
// documentation, and the only thing that makes it checkable later is the
// page it was read from.
#[test]
fn every_entry_names_the_documentation_page_it_was_read_from() {
    for entry in &card().entries {
        assert!(
            entry
                .docs
                .starts_with("https://docs.cloud.google.com/"),
            "{} cites {} rather than Google's documentation",
            entry.id.as_str(),
            entry.docs
        );
        if let (Some(released), Some(retires)) = (entry.released, entry.retires_on) {
            assert!(retires > released, "{} retires before release", entry.id.as_str());
        }
    }
}

fn day(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("a valid test date")
}

// Why: "currently supported" is decided by the documentation's dates, never
// by calling the model. The 2.5 family retires 2026-10-20; with a 30-day
// notice window it is supported on 2026-09-11 and withheld from 2026-09-20.
#[test]
fn support_follows_the_documented_retirement_with_a_notice_window() {
    let card = card();
    let flash = card
        .lookup_id("vertex-gemini-2.5-flash")
        .expect("2.5 flash is priced");
    assert_eq!(flash.retires_on, Some(day("2026-10-20")));
    assert!(flash.is_supported(day("2026-09-11")));
    assert!(!flash.is_supported(day("2026-09-21")));
    assert!(!flash.is_supported(day("2026-10-21")));

    let current = card
        .lookup_id("vertex-gemini-3.8-flash")
        .expect("3.8 flash is priced");
    assert!(current.is_supported(day("2026-09-11")));
    assert!(current.is_supported(day("2027-09-11")));
    assert_eq!(current.price_until, Some(day("2026-12-31")));
}

#[test]
fn deprecated_maas_models_are_unsupported_inside_their_notice_window() {
    let card = card();
    for id in [
        "qwen.qwen3-next-instruct",
        "qwen.qwen3-235b",
        "moonshotai.kimi-k2-thinking",
        "deepseek.v3.2",
        "openai.gpt-oss-20b",
    ] {
        let entry = card.lookup_id(id).unwrap_or_else(|| panic!("{id} is priced"));
        assert_eq!(entry.retires_on, Some(day("2026-10-21")), "{id}");
        assert!(entry.is_supported(day("2026-09-11")), "{id} on 2026-09-11");
        assert!(!entry.is_supported(day("2026-09-25")), "{id} on 2026-09-25");
    }
    let coder = card
        .lookup_id("qwen.qwen3-coder-480b")
        .expect("qwen3-coder is priced");
    assert!(coder.retires_on.is_none());
    assert!(coder.is_supported(day("2026-12-31")));
}

// Why: a preview model is served only by explicit opt-in, whatever the date.
#[test]
fn a_preview_entry_without_opt_in_is_never_supported() {
    let card = card();
    let mut entry = card
        .lookup_id("vertex-gemini-3.1-pro-preview")
        .expect("3.1 pro is priced")
        .clone();
    assert!(entry.is_supported(day("2026-09-11")));
    entry.allow_preview = false;
    assert!(!entry.is_supported(day("2026-09-11")));
}
