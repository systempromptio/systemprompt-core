//! Unit tests for the thought-signature cache: re-injection of Gemini
//! `thoughtSignature` values into tool_use blocks stripped by strict
//! Anthropic-protocol clients.

use std::time::Duration;

use systemprompt_identifiers::GatewayConversationId;

use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::canonical_response::{
    CanonicalResponse, CanonicalStopReason, CanonicalUsage,
};
use systemprompt_api::services::gateway::signature_cache::ThoughtSignatureCache;
use systemprompt_models::profile::WireProtocol;

fn cache() -> ThoughtSignatureCache {
    ThoughtSignatureCache::new(Duration::from_secs(60), 10)
}

const GEMINI: Option<WireProtocol> = Some(WireProtocol::Gemini);

fn conv() -> GatewayConversationId {
    GatewayConversationId::new_unchecked("ctx_00000000000000aa")
}

fn tool_use(id: &str, signature: Option<&str>) -> CanonicalContent {
    CanonicalContent::ToolUse {
        id: id.to_owned(),
        name: "lookup".to_owned(),
        input: serde_json::json!({"q": "x"}),
        signature: signature.map(str::to_owned),
    }
}

fn request_with(content: Vec<CanonicalContent>) -> CanonicalRequest {
    CanonicalRequest {
        model: "m".into(),
        system: None,
        messages: vec![CanonicalMessage {
            role: Role::Assistant,
            content,
        }],
        max_tokens: 10,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

fn response_with(content: Vec<CanonicalContent>) -> CanonicalResponse {
    CanonicalResponse {
        id: "msg_1".into(),
        model: "m".into(),
        content,
        stop_reason: Some(CanonicalStopReason::ToolUse),
        usage: CanonicalUsage::default(),
        grounding: None,
        code_execution: None,
        raw_finish_reason: None,
        ..Default::default()
    }
}

fn signature_of(request: &CanonicalRequest) -> Option<String> {
    request.messages.iter().find_map(|m| {
        m.content.iter().find_map(|c| match c {
            CanonicalContent::ToolUse { signature, .. } => signature.clone(),
            _ => None,
        })
    })
}

#[test]
fn hydrate_injects_cached_signature_when_none() {
    let cache = cache();
    cache.store(&conv(), "call_1", "sig-a");
    let mut request = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&conv(), &mut request, GEMINI);
    assert_eq!(signature_of(&request).as_deref(), Some("sig-a"));
}

#[test]
fn hydrate_passthrough_on_miss() {
    let cache = cache();
    let mut request = request_with(vec![tool_use("call_unknown", None)]);
    cache.hydrate_request(&conv(), &mut request, GEMINI);
    assert_eq!(signature_of(&request), None);
}

#[test]
fn inbound_signature_wins_and_rewarms() {
    let cache = cache();
    cache.store(&conv(), "call_1", "cached");
    let mut request = request_with(vec![tool_use("call_1", Some("client"))]);
    cache.hydrate_request(&conv(), &mut request, GEMINI);
    assert_eq!(signature_of(&request).as_deref(), Some("client"));
    assert_eq!(cache.lookup(&conv(), "call_1").as_deref(), Some("client"));
}

#[test]
fn ttl_expiry_drops_entry() {
    let cache = ThoughtSignatureCache::new(Duration::from_millis(1), 10);
    cache.store(&conv(), "call_1", "sig-a");
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(cache.lookup(&conv(), "call_1"), None);
}

#[test]
fn lookup_refreshes_ttl() {
    let cache = ThoughtSignatureCache::new(Duration::from_millis(60), 10);
    cache.store(&conv(), "call_1", "sig-a");
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(cache.lookup(&conv(), "call_1").as_deref(), Some("sig-a"));
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(cache.lookup(&conv(), "call_1").as_deref(), Some("sig-a"));
}

#[test]
fn eviction_at_capacity_drops_oldest() {
    let cache = ThoughtSignatureCache::new(Duration::from_secs(60), 2);
    cache.store(&conv(), "call_1", "a");
    std::thread::sleep(Duration::from_millis(2));
    cache.store(&conv(), "call_2", "b");
    std::thread::sleep(Duration::from_millis(2));
    cache.store(&conv(), "call_3", "c");
    assert_eq!(cache.lookup(&conv(), "call_1"), None);
    assert_eq!(cache.lookup(&conv(), "call_2").as_deref(), Some("b"));
    assert_eq!(cache.lookup(&conv(), "call_3").as_deref(), Some("c"));
}

#[test]
fn store_at_capacity_purges_expired_before_evicting() {
    let cache = ThoughtSignatureCache::new(Duration::from_millis(1), 2);
    cache.store(&conv(), "call_1", "a");
    cache.store(&conv(), "call_2", "b");
    std::thread::sleep(Duration::from_millis(5));
    cache.store(&conv(), "call_3", "c");
    assert_eq!(cache.lookup(&conv(), "call_3").as_deref(), Some("c"));
}

#[test]
fn store_from_response_caches_only_signed_tool_use() {
    let cache = cache();
    let response = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_signed", Some("sig-a")),
        tool_use("call_unsigned", None),
    ]);
    cache.store_from_response(&conv(), &response);
    assert_eq!(
        cache.lookup(&conv(), "call_signed").as_deref(),
        Some("sig-a")
    );
    assert_eq!(cache.lookup(&conv(), "call_unsigned"), None);
}

#[test]
fn response_signatures_survive_a_stripped_replay() {
    let cache = cache();
    cache.store_from_response(
        &conv(),
        &response_with(vec![tool_use("call_1", Some("sig-a"))]),
    );
    let mut replay = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&conv(), &mut replay, GEMINI);
    assert_eq!(signature_of(&replay).as_deref(), Some("sig-a"));
}

#[test]
fn signatures_are_scoped_to_their_conversation() {
    let cache = cache();
    cache.store(&conv(), "call_1", "sig-a");
    let other = GatewayConversationId::new_unchecked("ctx_00000000000000bb");
    assert_eq!(cache.lookup(&other, "call_1"), None);
    let mut request = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&other, &mut request, GEMINI);
    assert_eq!(signature_of(&request), None);
}

#[test]
fn hydration_is_identical_for_every_wire() {
    for wire in [
        GEMINI,
        Some(WireProtocol::Anthropic),
        Some(WireProtocol::OpenAiChat),
        None,
    ] {
        let cache = cache();
        cache.store(&conv(), "call_1", "sig-a");
        let mut request = request_with(vec![tool_use("call_1", None)]);
        cache.hydrate_request(&conv(), &mut request, wire);
        assert_eq!(signature_of(&request).as_deref(), Some("sig-a"));
    }
}


#[test]
fn only_signed_tool_use_blocks_make_a_response_worth_caching() {
    let unsigned = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_1", None),
    ]);
    assert_eq!(ThoughtSignatureCache::signed_tool_use_count(&unsigned), 0);

    let signed = response_with(vec![
        CanonicalContent::Text("hi".to_owned()),
        tool_use("call_1", Some("sig-a")),
        tool_use("call_2", None),
        tool_use("call_3", Some("sig-c")),
    ]);
    assert_eq!(ThoughtSignatureCache::signed_tool_use_count(&signed), 2);
}

fn counter_value(name: &str, label: (&str, &str), body: impl FnOnce()) -> Option<u64> {
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, body);
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find_map(|(composite, _, _, value)| {
            let key = composite.key();
            let matches_name = key.name() == name;
            let matches_label = key
                .labels()
                .any(|l| l.key() == label.0 && l.value() == label.1);
            match value {
                metrics_util::debugging::DebugValue::Counter(n)
                    if matches_name && matches_label =>
                {
                    Some(n)
                },
                _ => None,
            }
        })
}

#[test]
fn gemini_hydration_records_hit_and_miss() {
    let hits = counter_value(
        "gateway_signature_hydration_total",
        ("outcome", "hit"),
        || {
            let cache = cache();
            cache.store(&conv(), "call_1", "sig-a");
            let mut request = request_with(vec![tool_use("call_1", None)]);
            cache.hydrate_request(&conv(), &mut request, GEMINI);
        },
    );
    assert_eq!(hits, Some(1));

    let misses = counter_value(
        "gateway_signature_hydration_total",
        ("outcome", "miss"),
        || {
            let cache = cache();
            let mut request = request_with(vec![tool_use("call_unknown", None)]);
            cache.hydrate_request(&conv(), &mut request, GEMINI);
        },
    );
    assert_eq!(misses, Some(1));
}

#[test]
fn non_gemini_hydration_records_nothing() {
    for wire in [
        Some(WireProtocol::Anthropic),
        Some(WireProtocol::OpenAiChat),
        Some(WireProtocol::OpenAiResponses),
        None,
    ] {
        for outcome in ["hit", "miss"] {
            let recorded = counter_value(
                "gateway_signature_hydration_total",
                ("outcome", outcome),
                || {
                    let cache = cache();
                    cache.store(&conv(), "call_1", "sig-a");
                    let mut request =
                        request_with(vec![tool_use("call_1", None), tool_use("call_2", None)]);
                    cache.hydrate_request(&conv(), &mut request, wire);
                },
            );
            assert_eq!(recorded, None);
        }
    }
}

#[test]
fn uncacheable_response_records_only_when_signatures_are_present() {
    let signed = counter_value(
        "gateway_signature_capture_skipped_total",
        ("reason", "no_conversation_id"),
        || {
            let response = response_with(vec![tool_use("call_1", Some("sig-a"))]);
            ThoughtSignatureCache::note_uncacheable_response(&response, "no_conversation_id");
        },
    );
    assert_eq!(signed, Some(1));

    let unsigned = counter_value(
        "gateway_signature_capture_skipped_total",
        ("reason", "no_conversation_id"),
        || {
            let response = response_with(vec![
                CanonicalContent::Text("hi".to_owned()),
                tool_use("call_1", None),
            ]);
            ThoughtSignatureCache::note_uncacheable_response(&response, "no_conversation_id");
        },
    );
    assert_eq!(unsigned, None);
}

#[test]
fn cache_survives_a_poisoned_lock() {
    let cache = cache();
    cache.store(&conv(), "call_1", "sig-a");

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let poisoned =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cache.poison_lock())).is_err();
    std::panic::set_hook(previous);
    assert!(poisoned);

    assert_eq!(cache.lookup(&conv(), "call_1").as_deref(), Some("sig-a"));
    cache.store(&conv(), "call_2", "sig-b");
    assert_eq!(cache.lookup(&conv(), "call_2").as_deref(), Some("sig-b"));
}
