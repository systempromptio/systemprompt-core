use systemprompt_bridge::proxy::session::derive_context_id;

#[test]
fn anthropic_messages_shape_hashes() {
    let body = br#"{"system":"helpful","messages":[{"role":"user","content":"hi"}]}"#;
    let h = derive_context_id(body).expect("anthropic body should hash");
    assert_ne!(h, 0);
}

#[test]
fn openai_chat_shape_hashes() {
    let body =
        br#"{"messages":[{"role":"system","content":"sys"},{"role":"user","content":"hi"}]}"#;
    let h = derive_context_id(body).expect("openai chat body should hash");
    assert_ne!(h, 0);
}

#[test]
fn openai_responses_shape_hashes() {
    let body =
        br#"{"instructions":"helpful","input":[{"role":"user","content":[{"type":"input_text","text":"hi"}]}]}"#;
    let h = derive_context_id(body).expect("openai responses body should hash");
    assert_ne!(h, 0);
}

#[test]
fn anthropic_array_content_hashes() {
    let body =
        br#"{"messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]}"#;
    let h = derive_context_id(body).expect("anthropic array content should hash");
    assert_ne!(h, 0);
}

#[test]
fn malformed_body_returns_none() {
    assert!(derive_context_id(b"not json").is_none());
}

#[test]
fn empty_messages_returns_none() {
    assert!(derive_context_id(br#"{"messages":[]}"#).is_none());
}

#[test]
fn no_first_user_returns_none() {
    let body = br#"{"messages":[{"role":"system","content":"only system"}]}"#;
    assert!(derive_context_id(body).is_none());
}

#[test]
fn same_first_turn_hashes_stably() {
    let a = br#"{"system":"s","messages":[{"role":"user","content":"hello"}]}"#;
    let b = br#"{"system":"s","messages":[{"role":"user","content":"hello"}]}"#;
    assert_eq!(derive_context_id(a), derive_context_id(b));
}

#[test]
fn second_turn_does_not_change_hash() {
    let first = br#"{"system":"s","messages":[{"role":"user","content":"hello"}]}"#;
    let after_reply = br#"{"system":"s","messages":[{"role":"user","content":"hello"},{"role":"assistant","content":"hi"},{"role":"user","content":"again"}]}"#;
    assert_eq!(derive_context_id(first), derive_context_id(after_reply));
}

#[test]
fn different_first_turns_hash_differently() {
    let a = br#"{"messages":[{"role":"user","content":"alpha"}]}"#;
    let b = br#"{"messages":[{"role":"user","content":"beta"}]}"#;
    assert_ne!(derive_context_id(a), derive_context_id(b));
}
