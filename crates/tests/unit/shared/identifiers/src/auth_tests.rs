use systemprompt_identifiers::{JwtToken, SessionToken, CloudAuthToken, DbValue, ToDbValue};

#[test]
fn jwt_token_redacted_short_token() {
    let token = JwtToken::new("short");
    assert_eq!(token.redacted(), "*****");
}

#[test]
fn jwt_token_redacted_exactly_16_chars() {
    let token = JwtToken::new("1234567890123456");
    assert_eq!(token.redacted(), "********");
}

#[test]
fn jwt_token_redacted_long_token_shows_prefix_and_suffix() {
    let token = JwtToken::new("12345678abcdefghijklmnop");
    let redacted = token.redacted();
    assert!(redacted.starts_with("12345678"));
    assert!(redacted.ends_with("mnop"));
    assert!(redacted.contains("..."));
}

#[test]
fn jwt_token_display_uses_redacted() {
    let token = JwtToken::new("short-token");
    let display = format!("{}", token);
    assert_eq!(display, token.redacted());
    assert!(!display.contains("short-token"));
}

#[test]
fn jwt_token_display_never_leaks_full_value_long() {
    let long_token = "a]b".repeat(100);
    let token = JwtToken::new(&long_token);
    let display = format!("{}", token);
    assert_ne!(display, long_token);
    assert!(display.contains("..."));
}

#[test]
fn jwt_token_as_str_returns_full_value() {
    let raw = "eyJhbGciOiJIUzI1NiJ9.payload.signature";
    let token = JwtToken::new(raw);
    assert_eq!(token.as_str(), raw);
}

#[test]
fn jwt_token_serde_roundtrip_preserves_value() {
    let raw = "jwt-secret-value";
    let token = JwtToken::new(raw);
    let json = serde_json::to_string(&token).unwrap();
    assert_eq!(json, "\"jwt-secret-value\"");
    let deserialized: JwtToken = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.as_str(), raw);
}

#[test]
fn jwt_token_from_string_and_str() {
    let from_str: JwtToken = "value".into();
    let from_string: JwtToken = String::from("value").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn jwt_token_to_db_value() {
    let token = JwtToken::new("db-test");
    let db_val = token.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "db-test"));
}

#[test]
fn jwt_token_ref_to_db_value() {
    let token = JwtToken::new("ref-test");
    let db_val = (&token).to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "ref-test"));
}

#[test]
fn jwt_token_empty_redacted() {
    let token = JwtToken::new("");
    assert_eq!(token.redacted(), "");
}

#[test]
fn session_token_redacted_behavior() {
    let token = SessionToken::new("12345678901234567890");
    let redacted = token.redacted();
    assert!(redacted.starts_with("12345678"));
    assert!(redacted.ends_with("7890"));
    assert!(redacted.contains("..."));
}

#[test]
fn session_token_display_uses_redacted() {
    let token = SessionToken::new("secret-session-value");
    let display = format!("{}", token);
    assert!(!display.contains("secret-session-value"));
}

#[test]
fn session_token_as_str_returns_full_value() {
    let token = SessionToken::new("full-value");
    assert_eq!(token.as_str(), "full-value");
}

#[test]
fn cloud_auth_token_redacted_behavior() {
    let token = CloudAuthToken::new("12345678901234567890");
    let redacted = token.redacted();
    assert!(redacted.starts_with("12345678"));
    assert!(redacted.contains("..."));
}

#[test]
fn cloud_auth_token_display_does_not_leak() {
    let token = CloudAuthToken::new("super-secret-cloud-auth-token-value");
    let display = format!("{}", token);
    assert_ne!(display, "super-secret-cloud-auth-token-value");
}

#[test]
fn cloud_auth_token_serde_roundtrip() {
    let token = CloudAuthToken::new("cloud-val");
    let json = serde_json::to_string(&token).unwrap();
    assert_eq!(json, "\"cloud-val\"");
    let deserialized: CloudAuthToken = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.as_str(), "cloud-val");
}
