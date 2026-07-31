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

fn cache() -> ThoughtSignatureCache {
    ThoughtSignatureCache::new(Duration::from_secs(60), 10)
}

fn conv() -> GatewayConversationId {
    GatewayConversationId::new("ctx_00000000000000aa")
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
    cache.hydrate_request(&conv(), &mut request);
    assert_eq!(signature_of(&request).as_deref(), Some("sig-a"));
}

#[test]
fn hydrate_passthrough_on_miss() {
    let cache = cache();
    let mut request = request_with(vec![tool_use("call_unknown", None)]);
    cache.hydrate_request(&conv(), &mut request);
    assert_eq!(signature_of(&request), None);
}

#[test]
fn inbound_signature_wins_and_rewarms() {
    let cache = cache();
    cache.store(&conv(), "call_1", "cached");
    let mut request = request_with(vec![tool_use("call_1", Some("client"))]);
    cache.hydrate_request(&conv(), &mut request);
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
    cache.hydrate_request(&conv(), &mut replay);
    assert_eq!(signature_of(&replay).as_deref(), Some("sig-a"));
}

#[test]
fn signatures_are_scoped_to_their_conversation() {
    let cache = cache();
    cache.store(&conv(), "call_1", "sig-a");
    let other = GatewayConversationId::new("ctx_00000000000000bb");
    assert_eq!(cache.lookup(&other, "call_1"), None);
    let mut request = request_with(vec![tool_use("call_1", None)]);
    cache.hydrate_request(&other, &mut request);
    assert_eq!(signature_of(&request), None);
}
