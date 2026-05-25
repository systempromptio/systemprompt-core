use systemprompt_identifiers::{ContextId, GatewayConversationId};
use systemprompt_models::gateway_hash::conversation_prefix_hash;

use crate::support::minimal_request;

#[test]
fn context_id_derivation_is_deterministic_for_same_gateway_conversation_id() {
    let req = minimal_request(Some("you are helpful"), "hello world");
    let gw_a = req
        .derived_gateway_conversation_id()
        .expect("derives gateway id");
    let gw_b = req
        .derived_gateway_conversation_id()
        .expect("derives gateway id");
    assert_eq!(gw_a.as_str(), gw_b.as_str());

    let ctx_a = ContextId::derived_from_gateway_conversation(&gw_a);
    let ctx_b = ContextId::derived_from_gateway_conversation(&gw_b);
    assert_eq!(
        ctx_a.as_str(),
        ctx_b.as_str(),
        "same gateway conversation id must always map to the same ContextId"
    );

    let parsed = uuid::Uuid::parse_str(ctx_a.as_str()).expect("derived ContextId is a UUID");
    assert_eq!(
        parsed.get_version_num(),
        5,
        "derived ContextId must be UUID v5"
    );
}

#[test]
fn context_id_changes_when_system_prompt_changes_mid_conversation() {
    let baseline = minimal_request(Some("system prompt A"), "hello world");
    let rotated = minimal_request(Some("system prompt B — totally new"), "hello world");

    let gw_a = baseline.derived_gateway_conversation_id().unwrap();
    let gw_b = rotated.derived_gateway_conversation_id().unwrap();
    assert_ne!(
        gw_a.as_str(),
        gw_b.as_str(),
        "rotating the system prompt must yield a new GatewayConversationId"
    );

    let ctx_a = ContextId::derived_from_gateway_conversation(&gw_a);
    let ctx_b = ContextId::derived_from_gateway_conversation(&gw_b);
    assert_ne!(
        ctx_a.as_str(),
        ctx_b.as_str(),
        "system-prompt rotation must propagate to ContextId"
    );
}

#[test]
fn context_id_derivation_ignores_later_messages() {
    let mut a = minimal_request(Some("sys"), "first turn");
    let mut b = minimal_request(Some("sys"), "first turn");
    use systemprompt_api::services::gateway::protocol::canonical::{
        CanonicalContent, CanonicalMessage, Role,
    };
    b.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::Text("assistant reply".into())],
    });
    a.messages.push(CanonicalMessage {
        role: Role::Assistant,
        content: vec![CanonicalContent::Text("different assistant reply".into())],
    });

    let gw_a = a.derived_gateway_conversation_id().unwrap();
    let gw_b = b.derived_gateway_conversation_id().unwrap();
    assert_eq!(
        gw_a.as_str(),
        gw_b.as_str(),
        "only the first turn participates in the prefix hash"
    );
}

#[test]
fn derived_gateway_conversation_id_is_absent_for_empty_messages() {
    let mut req = minimal_request(Some("sys"), "x");
    req.messages.clear();
    assert!(
        req.derived_gateway_conversation_id().is_none(),
        "empty conversation must not derive a gateway id"
    );
}

#[test]
fn prefix_hash_handles_empty_system_prompt() {
    let with_empty = conversation_prefix_hash(Some(""), "user", "hello");
    let without = conversation_prefix_hash(None, "user", "hello");
    assert_eq!(
        with_empty, without,
        "empty system prompt is treated identically to absent system prompt"
    );
}

#[test]
fn prefix_hash_distinguishes_non_ascii_content() {
    // Multi-byte UTF-8 characters: ensure the hash sees byte-level content
    // and produces distinct values for visually-similar inputs.
    let h_emoji = conversation_prefix_hash(None, "user", "café 🚀");
    let h_ascii = conversation_prefix_hash(None, "user", "cafe");
    assert_ne!(h_emoji, h_ascii);
    let h_emoji_repeat = conversation_prefix_hash(None, "user", "café 🚀");
    assert_eq!(
        h_emoji, h_emoji_repeat,
        "hash must be deterministic across calls with identical UTF-8"
    );
}

#[test]
fn prefix_hash_changes_for_long_inputs() {
    let short = conversation_prefix_hash(None, "user", &"a".repeat(1_024));
    let long = conversation_prefix_hash(None, "user", &"a".repeat(65_536));
    assert_ne!(
        short, long,
        "length-prefix mixing must distinguish different-length inputs"
    );
}

#[test]
fn prefix_hash_domain_separates_system_role_content() {
    // If the hash naively concatenated segments without labels or length
    // delimiters, these two would collide. With label + length framing they
    // must not.
    let a = conversation_prefix_hash(Some("foo"), "bar", "baz");
    let b = conversation_prefix_hash(Some("foo"), "barbaz", "");
    let c = conversation_prefix_hash(Some("foobar"), "baz", "");
    let d = conversation_prefix_hash(Some(""), "foo", "barbaz");
    assert_ne!(a, b, "role/content boundary must be preserved");
    assert_ne!(a, c, "system/role boundary must be preserved");
    assert_ne!(a, d, "absent system distinguished from non-empty role");
    assert_ne!(b, c);
    assert_ne!(c, d);
}

#[test]
fn gateway_conversation_id_is_canonical_ctx_prefix() {
    let gw = GatewayConversationId::from_prefix_hash(0x1234_5678_9abc_def0);
    assert_eq!(gw.as_str(), "ctx_123456789abcdef0");
    let gw_zero = GatewayConversationId::from_prefix_hash(0);
    assert_eq!(
        gw_zero.as_str(),
        "ctx_0000000000000000",
        "hash zero must produce 16 hex chars, not be truncated"
    );
}
