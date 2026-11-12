//! Chain planning before dispatch: the selection-strategy attempt order and
//! the context-window pre-check with its fallbacks.

use systemprompt_catalog_fixtures::{CatalogFixture, model, platform, priced};
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_gateway::service::chain_plan::{
    ChainHealth, SentBody, fit_context_window, order_chain,
};
use systemprompt_gateway::service::failover::{
    DeploymentLoad, DeploymentState, ProviderBreakers, plan_selection,
};
use systemprompt_identifiers::{ModelId, RouteId, UserId};
use systemprompt_manifest::services::catalog::{
    EndpointConfig, ProviderConfig, ResolvedCatalog, ResolvedHop,
};
use systemprompt_manifest::services::{GatewayRoute, SelectionStrategy};
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

const fn state(tripped: bool, weight: u32, in_flight: u64) -> DeploymentState {
    DeploymentState {
        tripped,
        weight,
        in_flight,
    }
}

#[test]
fn ordered_keeps_chain_order_and_skips_tripped() {
    let states = [state(false, 1, 0), state(true, 1, 0), state(false, 1, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::Ordered, &states, 7),
        vec![0, 2]
    );
}

#[test]
fn weighted_draw_lands_by_cumulative_weight() {
    let states = [state(false, 1, 0), state(false, 3, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 0),
        vec![0, 1]
    );
    for draw in 1..4 {
        assert_eq!(
            plan_selection(SelectionStrategy::Weighted, &states, draw),
            vec![1, 0]
        );
    }
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 4),
        vec![0, 1]
    );
}

#[test]
fn weighted_never_draws_a_tripped_deployment_and_tries_it_last() {
    let states = [state(true, 100, 0), state(false, 1, 0), state(false, 2, 0)];
    for draw in 0..10 {
        let order = plan_selection(SelectionStrategy::Weighted, &states, draw);
        assert_ne!(order[0], 0, "draw {draw}");
        assert_eq!(order.last(), Some(&0));
        assert_eq!(order.len(), 3);
    }
}

#[test]
fn weighted_with_every_deployment_tripped_still_sends() {
    let states = [state(true, 1, 0), state(true, 1, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::Weighted, &states, 3),
        vec![0, 1]
    );
}

#[test]
fn least_busy_leads_with_the_fewest_in_flight_and_ties_go_to_chain_order() {
    let states = [state(false, 1, 4), state(false, 1, 1), state(false, 1, 1)];
    assert_eq!(
        plan_selection(SelectionStrategy::LeastBusy, &states, 0),
        vec![1, 0, 2]
    );
    let tripped_idle = [state(false, 1, 9), state(true, 1, 0)];
    assert_eq!(
        plan_selection(SelectionStrategy::LeastBusy, &tripped_idle, 0),
        vec![0, 1]
    );
}

fn catalog() -> ResolvedCatalog {
    let mut fixture = CatalogFixture::new();
    for name in [
        "small",
        "large",
        "lb-plan-a",
        "lb-plan-b",
        "lb-plan-c",
        "w-plan-a",
        "w-plan-b",
        "w-plan-solo",
        "unregistered",
    ] {
        fixture = fixture
            .credential(name, platform(name))
            .provider(name, ProviderConfig::new(name))
            .endpoint(
                name,
                "api",
                EndpointConfig::new(WireProtocol::Anthropic, "https://example.test/v1"),
            );
    }
    fixture
        .model(
            "small-m",
            model(&[ClientFamily::Claude], 100, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "large-m",
            model(&[ClientFamily::Claude], 100_000, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "open-m",
            model(&[ClientFamily::Claude], 0, priced(1.0, 1.0, 0.0)),
        )
        .model(
            "plan-m",
            model(&[ClientFamily::Claude], 100_000, priced(1.0, 1.0, 0.0)),
        )
        .deploy("small-m", ["small/api"])
        .deploy("large-m", ["large/api"])
        .deploy("open-m", ["unregistered/api"])
        .deploy(
            "plan-m",
            [
                "lb-plan-a/api",
                "lb-plan-b/api",
                "lb-plan-c/api",
                "w-plan-a/api",
                "w-plan-b/api",
                "w-plan-solo/api",
            ],
        )
        .build()
}

fn hop(catalog: &ResolvedCatalog, model: &str, provider: &str, weight: Option<u32>) -> ResolvedHop {
    let hop = catalog
        .deployments(model)
        .iter()
        .find(|h| h.provider.as_str() == provider)
        .unwrap_or_else(|| panic!("{model} on {provider}"));
    ResolvedHop {
        weight,
        ..hop.clone()
    }
}

fn route_id() -> RouteId {
    RouteId::new("r")
}

#[test]
fn hop_weights_default_to_one() {
    let c = catalog();
    let weights: Vec<u32> = [
        hop(&c, "plan-m", "w-plan-a", Some(3)),
        hop(&c, "plan-m", "w-plan-b", Some(2)),
        hop(&c, "plan-m", "lb-plan-a", None),
    ]
    .iter()
    .map(ResolvedHop::effective_weight)
    .collect();
    assert_eq!(weights, vec![3, 2, 1]);
}

#[test]
fn ordered_chain_is_unchanged_and_not_described() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "small-m", "small", None),
            hop(&c, "large-m", "large", None),
        ],
        SelectionStrategy::Ordered,
        Some(&route_id()),
        ChainHealth {
            breakers: &ProviderBreakers::new(),
            load: &DeploymentLoad::default(),
            owner: &UserId::new("member-a"),
        },
    );
    assert_eq!(providers_of(&planned), vec!["small", "large"]);
    assert_eq!(planned.descriptor, None);
}

#[test]
fn a_bare_route_serializes_without_strategy_or_fallbacks() {
    let r = GatewayRoute::new("r");
    let yaml = serde_yaml::to_string(&r).expect("serialize");
    assert!(!yaml.contains("strategy") && !yaml.contains("weight") && !yaml.contains("context"));
}

fn reencoded() -> SentBody {
    SentBody::measure(None, b"")
}

fn request_of(chars: usize) -> CanonicalRequest {
    CanonicalRequest {
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("x".repeat(chars))],
        }],
        ..CanonicalRequest::new(ModelId::new("m"), Vec::new(), 64)
    }
}

#[test]
fn a_request_that_fits_keeps_its_chain() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(40),
        reencoded(),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("fits");
    assert_eq!(planned.deployments[0].provider.as_str(), "small");
    assert_eq!(planned.descriptor, None);
}

#[test]
fn an_oversized_request_moves_to_the_context_fallback() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        reencoded(),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("falls back");
    assert_eq!(planned.deployments.len(), 1);
    assert_eq!(planned.deployments[0].provider.as_str(), "large");
    assert_eq!(
        planned.descriptor.as_deref(),
        Some("context_window:small/api->large/api")
    );
}

fn billed_as(
    mut hop: ResolvedHop,
    kind: systemprompt_manifest::services::catalog::CredentialKind,
) -> ResolvedHop {
    hop.credential.kind = kind;
    hop.credential.secret = None;
    hop
}

#[test]
fn a_member_paid_request_never_moves_to_a_platform_context_fallback() {
    use systemprompt_manifest::services::catalog::CredentialKind;
    let c = catalog();
    for kind in [CredentialKind::Caller, CredentialKind::Personal] {
        let exceeded = fit_context_window(
            &c,
            &request_of(4_000),
            reencoded(),
            vec![billed_as(hop(&c, "small-m", "small", None), kind)],
            vec![hop(&c, "large-m", "large", None)],
        )
        .expect_err("a platform fallback would move the bill");
        assert_eq!(exceeded.limit, 100, "{kind:?}");
    }
}

#[test]
fn failover_stays_on_the_bill_the_request_started_on() {
    use systemprompt_manifest::services::catalog::CredentialKind;
    let c = catalog();
    let platform_a = hop(&c, "small-m", "small", None);
    let platform_b = hop(&c, "large-m", "large", None);
    let caller_a = billed_as(platform_a.clone(), CredentialKind::Caller);
    let caller_b = billed_as(platform_b.clone(), CredentialKind::Caller);

    assert!(platform_a.may_fail_over_to(&platform_b));
    assert!(!caller_a.may_fail_over_to(&platform_b));
    assert!(!platform_a.may_fail_over_to(&caller_b));
    assert!(
        !caller_a.may_fail_over_to(&caller_b),
        "two different credentials"
    );
    assert!(caller_a.may_fail_over_to(&caller_a.clone()));
}

#[test]
fn an_oversized_request_with_no_fitting_fallback_is_refused_with_the_numbers() {
    let c = catalog();
    let exceeded = fit_context_window(
        &c,
        &request_of(4_000),
        reencoded(),
        vec![hop(&c, "small-m", "small", None)],
        Vec::new(),
    )
    .expect_err("refused");
    assert_eq!(exceeded.limit, 100);
    assert!(exceeded.estimate > 100);
    assert!(exceeded.to_string().starts_with("context_window_exceeded"));
}

#[test]
fn failover_deployments_too_small_for_the_request_are_dropped() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        reencoded(),
        vec![
            hop(&c, "large-m", "large", None),
            hop(&c, "small-m", "small", None),
        ],
        Vec::new(),
    )
    .expect("fits the primary");
    assert_eq!(providers_of(&planned), vec!["large"]);
}

fn providers_of(planned: &systemprompt_gateway::service::chain_plan::PlannedChain) -> Vec<&str> {
    planned
        .deployments
        .iter()
        .map(|v| v.provider.as_str())
        .collect()
}

#[test]
fn an_empty_chain_plans_to_nothing() {
    let planned = order_chain(
        Vec::new(),
        SelectionStrategy::Ordered,
        Some(&route_id()),
        ChainHealth {
            breakers: &ProviderBreakers::new(),
            load: &DeploymentLoad::default(),
            owner: &UserId::new("member-a"),
        },
    );
    assert!(planned.deployments.is_empty());
    assert_eq!(planned.descriptor, None);
}

#[test]
fn an_idle_least_busy_chain_keeps_chain_order_and_is_described() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "plan-m", "lb-plan-a", None),
            hop(&c, "plan-m", "lb-plan-b", None),
            hop(&c, "plan-m", "lb-plan-c", None),
        ],
        SelectionStrategy::LeastBusy,
        Some(&route_id()),
        ChainHealth {
            breakers: &ProviderBreakers::new(),
            load: &DeploymentLoad::default(),
            owner: &UserId::new("member-a"),
        },
    );
    assert_eq!(
        providers_of(&planned),
        vec!["lb-plan-a", "lb-plan-b", "lb-plan-c"]
    );
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:least_busy"));
}

#[test]
fn a_weighted_chain_is_a_permutation_of_its_deployments() {
    let c = catalog();
    let planned = order_chain(
        vec![
            hop(&c, "plan-m", "w-plan-a", Some(5)),
            hop(&c, "plan-m", "w-plan-b", Some(1)),
        ],
        SelectionStrategy::Weighted,
        Some(&route_id()),
        ChainHealth {
            breakers: &ProviderBreakers::new(),
            load: &DeploymentLoad::default(),
            owner: &UserId::new("member-a"),
        },
    );
    let mut providers = providers_of(&planned);
    providers.sort_unstable();
    assert_eq!(providers, vec!["w-plan-a", "w-plan-b"]);
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:weighted"));
}

#[test]
fn a_single_weighted_deployment_is_not_reordered_but_is_described() {
    let c = catalog();
    let planned = order_chain(
        vec![hop(&c, "plan-m", "w-plan-solo", None)],
        SelectionStrategy::Weighted,
        Some(&route_id()),
        ChainHealth {
            breakers: &ProviderBreakers::new(),
            load: &DeploymentLoad::default(),
            owner: &UserId::new("member-a"),
        },
    );
    assert_eq!(providers_of(&planned), vec!["w-plan-solo"]);
    assert_eq!(planned.descriptor.as_deref(), Some("strategy:weighted"));
}

#[test]
fn a_deployment_with_no_known_window_is_never_refused() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(4_000),
        reencoded(),
        vec![
            hop(&c, "open-m", "unregistered", None),
            hop(&c, "small-m", "small", None),
        ],
        Vec::new(),
    )
    .expect("an unknown window is not a refusal");
    assert_eq!(providers_of(&planned), vec!["unregistered"]);
}

fn anthropic_body(block: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model": "m",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": [block, {"type": "text", "text": "hi"}]}]
    }))
    .expect("serialize")
}

fn text_document(chars: usize) -> SentBody {
    SentBody::measure(
        Some(WireProtocol::Anthropic),
        &anthropic_body(serde_json::json!({"type": "document", "source": {
            "type": "text", "media_type": "text/plain", "data": "x".repeat(chars)
        }})),
    )
}

#[test]
fn unmodelled_passthrough_text_counts_toward_the_context_window() {
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(40),
        text_document(4_000),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("falls back to the larger window");
    assert_eq!(providers_of(&planned), vec!["large"]);
    assert_eq!(
        planned.descriptor.as_deref(),
        Some("context_window:small/api->large/api")
    );
}

#[test]
fn a_huge_text_document_with_no_fitting_fallback_is_refused() {
    let c = catalog();
    let exceeded = fit_context_window(
        &c,
        &request_of(40),
        text_document(4_000),
        vec![hop(&c, "small-m", "small", None)],
        Vec::new(),
    )
    .expect_err("refused");
    assert_eq!(exceeded.limit, 100);
    assert!(exceeded.estimate > 1_000);
}

#[test]
fn passthrough_bytes_are_not_counted_on_a_hop_that_re_encodes() {
    let c = catalog();
    let raw = anthropic_body(serde_json::json!({"type": "text", "text": "x".repeat(4_000)}));
    let planned = fit_context_window(
        &c,
        &request_of(40),
        SentBody::measure(Some(WireProtocol::OpenAiChat), &raw),
        vec![hop(&c, "small-m", "small", None)],
        vec![hop(&c, "large-m", "large", None)],
    )
    .expect("fits");
    assert_eq!(providers_of(&planned), vec!["small"]);
    assert_eq!(planned.descriptor, None);
}

fn plans_onto_small(raw: &[u8], wire: WireProtocol) {
    assert_eq!(
        wire,
        WireProtocol::Anthropic,
        "the fixture hops are Anthropic"
    );
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(40),
        SentBody::measure(Some(wire), raw),
        vec![hop(&c, "small-m", "small", None)],
        Vec::new(),
    )
    .expect("base64 binary sources are not priced as text bytes");
    assert_eq!(providers_of(&planned), vec!["small"]);
}

#[test]
fn a_multi_megabyte_base64_pdf_still_plans_onto_the_deployment() {
    let raw = anthropic_body(serde_json::json!({"type": "document", "source": {
        "type": "base64", "media_type": "application/pdf", "data": "J".repeat(5 * 1024 * 1024)
    }}));
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn a_base64_image_is_not_priced_as_text_bytes() {
    let raw = anthropic_body(serde_json::json!({"type": "image", "source": {
        "type": "base64", "media_type": "image/png", "data": "A".repeat(400_000)
    }}));
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn openai_chat_file_and_data_uri_images_are_not_priced_as_text_bytes() {
    let data = "B".repeat(400_000);
    let raw = serde_json::to_vec(&serde_json::json!({
        "model": "m",
        "messages": [{"role": "user", "content": [
            {"type": "file", "file": {"filename": "a.pdf",
                "file_data": format!("data:application/pdf;base64,{data}")}},
            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{data}")}},
            {"type": "text", "text": "hi"}
        ]}]
    }))
    .expect("serialize");
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn responses_input_file_and_input_image_are_not_priced_as_text_bytes() {
    let data = "C".repeat(400_000);
    let raw = serde_json::to_vec(&serde_json::json!({
        "model": "m",
        "input": [{"role": "user", "content": [
            {"type": "input_file", "filename": "a.pdf", "file_data": data},
            {"type": "input_image", "image_url": format!("data:image/png;base64,{data}")},
            {"type": "input_text", "text": "hi"}
        ]}]
    }))
    .expect("serialize");
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn gemini_inline_data_is_not_priced_as_text_bytes() {
    let data = "D".repeat(400_000);
    let raw = serde_json::to_vec(&serde_json::json!({
        "contents": [{"role": "user", "parts": [
            {"inlineData": {"mimeType": "image/png", "data": data}},
            {"inline_data": {"mime_type": "image/png", "data": data}},
            {"fileData": {"mimeType": "application/pdf", "fileUri": "gs://b/a.pdf"}},
            {"text": "hi"}
        ]}]
    }))
    .expect("serialize");
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn openai_input_audio_is_not_priced_as_text_bytes() {
    let raw = serde_json::to_vec(&serde_json::json!({
        "model": "m",
        "messages": [{"role": "user", "content": [
            {"type": "input_audio", "input_audio": {"data": "E".repeat(400_000), "format": "wav"}},
            {"type": "text", "text": "hi"}
        ]}]
    }))
    .expect("serialize");
    plans_onto_small(&raw, WireProtocol::Anthropic);
}

#[test]
fn escaped_text_is_measured_by_its_decoded_length() {
    let text = "\u{4e2d}".repeat(90);
    let escaped = format!(
        r#"{{"model":"m","messages":[{{"role":"user","content":"{}"}}]}}"#,
        "\\u4e2d".repeat(90)
    );
    assert!(escaped.len() > 4 * 100);
    let c = catalog();
    let planned = fit_context_window(
        &c,
        &request_of(text.len()),
        SentBody::measure(Some(WireProtocol::Anthropic), escaped.as_bytes()),
        vec![hop(&c, "small-m", "small", None)],
        Vec::new(),
    )
    .expect("the decoded text fits the window");
    assert_eq!(providers_of(&planned), vec!["small"]);
}
