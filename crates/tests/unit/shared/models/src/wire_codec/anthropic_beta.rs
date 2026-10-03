//! `anthropic-beta` as typed values: the header a client sends is parsed into
//! its flags, narrowed by the policy the upstream's (wire, hosting) resolves,
//! and rendered back — or omitted when nothing is left.

use std::collections::BTreeSet;

use systemprompt_wire::anthropic::{AnthropicBeta, BetaHeader, BetaPolicy};
use systemprompt_wire::upstream::UpstreamDialect;
use systemprompt_wire::{Hosting, WireProtocol};

fn only(flags: &[&str]) -> BetaPolicy {
    BetaPolicy::Only(flags.iter().map(|f| AnthropicBeta::new(*f)).collect())
}

#[test]
fn a_header_is_parsed_trimmed_deduplicated_and_ordered_as_sent() {
    let header = BetaHeader::parse(" b-2025 , a-2025,, b-2025 ");
    let flags: Vec<&str> = header.betas().iter().map(AnthropicBeta::as_str).collect();
    assert_eq!(flags, ["b-2025", "a-2025"]);
    assert_eq!(header.render().as_deref(), Some("b-2025,a-2025"));
}

#[test]
fn an_empty_header_renders_nothing() {
    assert!(BetaHeader::parse(" , ").is_empty());
    assert_eq!(BetaHeader::parse("").render(), None);
}

#[test]
fn a_policy_keeps_only_what_it_admits() {
    let kept = BetaHeader::parse("a-2025,b-2025,c-2025").admitted_by(&only(&["c-2025", "a-2025"]));
    assert_eq!(kept.render().as_deref(), Some("a-2025,c-2025"));
    assert_eq!(
        BetaHeader::parse("a-2025").admitted_by(&only(&[])).render(),
        None
    );
    assert_eq!(
        BetaHeader::parse("a-2025")
            .admitted_by(&BetaPolicy::ForwardAll)
            .render()
            .as_deref(),
        Some("a-2025")
    );
}

#[test]
fn the_dialect_resolves_the_default_policy_from_the_hosting() {
    let first_party = UpstreamDialect::new(WireProtocol::Anthropic, Hosting::FirstParty);
    let vertex = UpstreamDialect::new(WireProtocol::Anthropic, Hosting::Vertex);
    assert_eq!(first_party.beta_policy(None), BetaPolicy::ForwardAll);
    assert_eq!(vertex.beta_policy(None), BetaPolicy::Only(BTreeSet::new()));

    let declared: BTreeSet<AnthropicBeta> = [AnthropicBeta::new("a-2025")].into();
    assert_eq!(
        vertex.beta_policy(Some(&declared)),
        BetaPolicy::Only(declared.clone())
    );
    assert_eq!(
        first_party.beta_policy(Some(&declared)),
        BetaPolicy::Only(declared)
    );
}

#[test]
fn a_provider_declares_its_betas_as_a_yaml_list() {
    let yaml = "name: vertex-anthropic\nwire: anthropic\nsurface: anthropic\n\
                endpoint: https://aiplatform.googleapis.com/v1/projects/{project}/locations/global/publishers/anthropic\n\
                api_key_secret: vertex\naccepted_betas: [a-2025, b-2025]\n";
    let entry: systemprompt_models::services::ProviderEntry =
        serde_yaml::from_str(yaml).expect("provider parses");
    let declared = entry.accepted_betas.expect("declared");
    assert!(declared.contains(&AnthropicBeta::new("a-2025")));
    assert_eq!(declared.len(), 2);
}

#[test]
fn the_refused_flags_are_the_complement_of_the_admitted_ones() {
    let policy = only(&["a-2025"]);
    let sent = BetaHeader::parse("a-2025,b-2025,c-2025");
    assert_eq!(
        sent.clone().admitted_by(&policy).render().as_deref(),
        Some("a-2025")
    );
    assert_eq!(
        sent.refused_by(&policy).render().as_deref(),
        Some("b-2025,c-2025")
    );
    assert!(
        BetaHeader::parse("a-2025")
            .refused_by(&BetaPolicy::ForwardAll)
            .is_empty()
    );
}

#[test]
fn extending_a_header_keeps_each_flag_once() {
    let mut header = BetaHeader::parse("a-2025,b-2025");
    header.extend(BetaHeader::parse("b-2025,c-2025"));
    assert_eq!(header.render().as_deref(), Some("a-2025,b-2025,c-2025"));
    assert!(header.contains("c-2025"));
    assert!(!header.contains("d-2025"));
}

#[test]
fn a_dropped_beta_takes_the_field_it_gates_with_it() {
    use systemprompt_wire::anthropic::strip_fields_gated_by;

    let body = || {
        serde_json::json!({
            "max_tokens": 8,
            "messages": [],
            "context_management": { "edits": [{ "type": "clear_thinking_20251015" }] },
            "mcp_servers": [{ "type": "url", "url": "https://mcp.example" }],
            "output_config": { "effort": "medium" }
        })
        .as_object()
        .cloned()
        .expect("object")
    };

    // Every version of the beta opens the field: the date suffix is not matched.
    let mut obj = body();
    let removed = strip_fields_gated_by(
        &mut obj,
        &BetaHeader::parse("context-management-2099-01-01"),
    );
    assert_eq!(removed, ["context_management"]);
    assert!(obj.get("context_management").is_none());
    assert!(
        obj.get("mcp_servers").is_some(),
        "mcp-client was not dropped"
    );
    assert!(
        obj.get("output_config").is_some(),
        "not beta-gated, never touched"
    );

    // Nothing dropped, nothing removed — a field sent without its flag is the
    // client's own contract with the upstream.
    let mut obj = body();
    assert!(strip_fields_gated_by(&mut obj, &BetaHeader::default()).is_empty());
    assert_eq!(obj.len(), 5);

    // A dropped beta whose field is absent reports nothing.
    let mut obj = body();
    obj.remove("mcp_servers");
    assert!(
        strip_fields_gated_by(&mut obj, &BetaHeader::parse("mcp-client-2025-04-04")).is_empty()
    );
}

#[test]
fn the_gated_field_table_pairs_each_field_with_one_beta_prefix() {
    use systemprompt_wire::anthropic::BETA_GATED_FIELDS;

    let fields: BTreeSet<&str> = BETA_GATED_FIELDS.iter().map(|g| g.field).collect();
    assert_eq!(fields.len(), BETA_GATED_FIELDS.len(), "one entry per field");
    assert!(fields.contains("context_management"));
    for gate in BETA_GATED_FIELDS {
        assert!(
            gate.beta_prefix.ends_with('-'),
            "{}: the prefix stops before the date so every version matches",
            gate.beta_prefix
        );
    }
}
