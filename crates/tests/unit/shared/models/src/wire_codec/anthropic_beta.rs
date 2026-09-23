//! `anthropic-beta` as typed values: the header a client sends is parsed into
//! its flags, narrowed by the policy the upstream's (wire, hosting) resolves,
//! and rendered back — or omitted when nothing is left.

use std::collections::BTreeSet;

use systemprompt_models::services::{Hosting, WireProtocol};
use systemprompt_models::wire::anthropic::{AnthropicBeta, BetaHeader, BetaPolicy};
use systemprompt_models::wire::upstream::UpstreamDialect;

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
