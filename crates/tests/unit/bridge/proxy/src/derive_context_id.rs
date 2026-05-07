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
    let body = br#"{"messages":[{"role":"user","content":[{"type":"text","text":"hello"}]}]}"#;
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

#[test]
fn anthropic_and_openai_chat_with_same_system_hash_equal() {
    let anthropic = br#"{"system":"helpful","messages":[{"role":"user","content":"hi"}]}"#;
    let openai_chat =
        br#"{"messages":[{"role":"system","content":"helpful"},{"role":"user","content":"hi"}]}"#;
    assert_eq!(
        derive_context_id(anthropic),
        derive_context_id(openai_chat),
        "same conversation in two wire formats should hash equal"
    );
}

#[test]
fn openai_responses_instructions_align_with_anthropic_system() {
    let anthropic = br#"{"system":"helpful","messages":[{"role":"user","content":"hi"}]}"#;
    let openai_responses =
        br#"{"instructions":"helpful","input":[{"role":"user","content":[{"type":"input_text","text":"hi"}]}]}"#;
    assert_eq!(
        derive_context_id(anthropic),
        derive_context_id(openai_responses),
        "anthropic system + openai instructions should converge"
    );
}

#[test]
fn role_changes_hash() {
    let user_first = br#"{"messages":[{"role":"user","content":"hi"}]}"#;
    let assistant_first = br#"{"messages":[{"role":"assistant","content":"hi"}]}"#;
    assert_ne!(
        derive_context_id(user_first),
        derive_context_id(assistant_first)
    );
}

#[test]
fn unicode_content_hashes() {
    let body = "{\"messages\":[{\"role\":\"user\",\"content\":\"こんにちは 🌸\"}]}".as_bytes();
    let h = derive_context_id(body).expect("unicode body should hash");
    assert_ne!(h, 0);
}

#[test]
fn long_content_hashes_stably() {
    let long = "x".repeat(64 * 1024);
    let body = format!(r#"{{"messages":[{{"role":"user","content":"{long}"}}]}}"#);
    let h1 = derive_context_id(body.as_bytes()).expect("long body should hash");
    let h2 = derive_context_id(body.as_bytes()).expect("long body should hash");
    assert_eq!(h1, h2);
}

#[test]
fn array_content_concatenates_text_parts() {
    let split = br#"{"messages":[{"role":"user","content":[{"type":"text","text":"hello"},{"type":"text","text":"world"}]}]}"#;
    let joined = br#"{"messages":[{"role":"user","content":"hello\nworld"}]}"#;
    assert_eq!(
        derive_context_id(split),
        derive_context_id(joined),
        "array text parts should concatenate to the same hash as a newline-joined string"
    );
}

#[test]
fn multiple_system_messages_concatenate() {
    let one =
        br#"{"messages":[{"role":"system","content":"a\nb"},{"role":"user","content":"hi"}]}"#;
    let two = br#"{"messages":[{"role":"system","content":"a"},{"role":"system","content":"b"},{"role":"user","content":"hi"}]}"#;
    assert_eq!(
        derive_context_id(one),
        derive_context_id(two),
        "multiple system messages should concatenate into a single canonical system"
    );
}

#[test]
fn anthropic_array_system_value_serializes_consistently() {
    let body =
        br#"{"system":[{"type":"text","text":"hi"}],"messages":[{"role":"user","content":"hello"}]}"#;
    let h1 = derive_context_id(body).expect("array system should hash");
    let h2 = derive_context_id(body).expect("array system should hash");
    assert_eq!(h1, h2);
}

#[test]
fn empty_string_content_still_hashes() {
    let body = br#"{"messages":[{"role":"user","content":""}]}"#;
    let h = derive_context_id(body).expect("empty content should still hash");
    assert_ne!(h, 0);
}

#[test]
fn missing_role_defaults_to_user() {
    let no_role = br#"{"messages":[{"content":"hi"}]}"#;
    let user_role = br#"{"messages":[{"role":"user","content":"hi"}]}"#;
    assert_eq!(
        derive_context_id(no_role),
        derive_context_id(user_role),
        "absent role should be treated as 'user'"
    );
}

#[test]
fn extra_unknown_fields_dont_affect_hash() {
    let bare = br#"{"messages":[{"role":"user","content":"hi"}]}"#;
    let with_extras =
        br#"{"model":"x","temperature":0.7,"max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#;
    assert_eq!(derive_context_id(bare), derive_context_id(with_extras));
}
