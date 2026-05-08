use systemprompt_identifiers::GatewayConversationId;

#[test]
fn from_prefix_hash_emits_canonical_format() {
    let id = GatewayConversationId::from_prefix_hash(0xdead_beef_cafe_babe);
    assert_eq!(id.as_str(), "ctx_deadbeefcafebabe");
}

#[test]
fn from_prefix_hash_zero_pads() {
    let id = GatewayConversationId::from_prefix_hash(1);
    assert_eq!(id.as_str(), "ctx_0000000000000001");
}

#[test]
fn try_new_accepts_canonical_form() {
    GatewayConversationId::try_new("ctx_0000000000000001").expect("valid id");
}

#[test]
fn try_new_rejects_uppercase() {
    assert!(GatewayConversationId::try_new("ctx_DEADBEEFCAFEBABE").is_err());
}

#[test]
fn try_new_rejects_missing_prefix() {
    assert!(GatewayConversationId::try_new("0000000000000001").is_err());
}

#[test]
fn try_new_rejects_wrong_length() {
    assert!(GatewayConversationId::try_new("ctx_dead").is_err());
    assert!(GatewayConversationId::try_new("ctx_deadbeefcafebabeextra").is_err());
}

#[test]
fn try_new_rejects_non_hex_suffix() {
    assert!(GatewayConversationId::try_new("ctx_deadbeefcafebabz").is_err());
}

#[test]
fn equal_hashes_produce_equal_ids() {
    let a = GatewayConversationId::from_prefix_hash(42);
    let b = GatewayConversationId::from_prefix_hash(42);
    assert_eq!(a, b);
}
