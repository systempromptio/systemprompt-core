use systemprompt_bridge::gateway::GatewayRejection;

#[test]
fn an_api_error_body_keeps_its_code_key_and_message() {
    let rejection = GatewayRejection::parse(
        r#"{"code":"conflict_error","message":"device already enrolled","error_key":"unique_violation"}"#,
    );
    assert_eq!(rejection.code.as_deref(), Some("conflict_error"));
    assert_eq!(rejection.error_key.as_deref(), Some("unique_violation"));
    assert_eq!(
        rejection.message.as_deref(),
        Some("device already enrolled")
    );
    assert_eq!(rejection.to_string(), "device already enrolled");
}

#[test]
fn an_oauth_error_body_maps_error_to_code() {
    let rejection =
        GatewayRejection::parse(r#"{"error":"invalid_grant","error_description":"expired"}"#);
    assert_eq!(rejection.code.as_deref(), Some("invalid_grant"));
    assert_eq!(rejection.message.as_deref(), Some("expired"));
    assert_eq!(rejection.error_key, None);
}

#[test]
fn a_plain_text_body_is_kept_as_a_bounded_excerpt() {
    let rejection = GatewayRejection::parse(&format!("  {}  ", "y".repeat(500)));
    assert_eq!(rejection.code, None);
    assert_eq!(rejection.message, None);
    assert_eq!(rejection.excerpt.chars().count(), 240);
    assert_eq!(rejection.to_string(), rejection.excerpt);
}

#[test]
fn an_empty_body_says_so() {
    let rejection = GatewayRejection::parse("   ");
    assert!(rejection.excerpt.is_empty());
    assert_eq!(rejection.to_string(), "no response body");
}
