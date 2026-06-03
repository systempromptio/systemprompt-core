use std::str::FromStr;

use systemprompt_bridge::ids::{
    BearerToken, LoopbackSecret, PatToken, PinnedPubKey, ProxySecret,
};

const LONG_SECRET: &str = "sp-live-0123456789abcdef";
const SHORT_SECRET: &str = "short";

#[test]
fn redacted_long_token_shows_first8_dots_last4() {
    let token = PatToken::new(LONG_SECRET);
    assert!(LONG_SECRET.len() > 16);
    assert_eq!(token.redacted(), "sp-live-...cdef");
}

#[test]
fn redacted_short_token_shows_fixed_mask() {
    let token = PatToken::new(SHORT_SECRET);
    assert!(SHORT_SECRET.len() <= 16);
    assert_eq!(token.redacted(), "***");
}

#[test]
fn redacted_boundary_16_chars_is_masked() {
    let exactly_16 = "0123456789abcdef";
    assert_eq!(exactly_16.len(), 16);
    let token = BearerToken::new(exactly_16);
    assert_eq!(token.redacted(), "***");
}

#[test]
fn redacted_boundary_17_chars_is_partial() {
    let seventeen = "0123456789abcdefX";
    assert_eq!(seventeen.len(), 17);
    let token = BearerToken::new(seventeen);
    assert_eq!(token.redacted(), "01234567...defX");
}

#[test]
fn debug_shows_type_and_redacted_never_raw() {
    let token = PatToken::new(LONG_SECRET);
    let debug = format!("{token:?}");
    assert_eq!(debug, format!("PatToken({})", token.redacted()));
    assert!(!debug.contains(LONG_SECRET));
    assert!(!debug.contains("0123456789abcdef"));
}

#[test]
fn display_shows_redacted_never_raw() {
    let token = PatToken::new(LONG_SECRET);
    let display = format!("{token}");
    assert_eq!(display, token.redacted());
    assert!(!display.contains(LONG_SECRET));
}

#[test]
fn as_str_returns_raw_value() {
    let token = LoopbackSecret::new(LONG_SECRET);
    assert_eq!(token.as_str(), LONG_SECRET);
}

#[test]
fn new_as_str_round_trip() {
    let token = ProxySecret::new(SHORT_SECRET);
    assert_eq!(token.as_str(), SHORT_SECRET);
}

#[test]
fn into_inner_returns_raw_string() {
    let token = PinnedPubKey::new(LONG_SECRET);
    let inner = token.into_inner();
    assert_eq!(inner, LONG_SECRET);
}

#[test]
fn serde_serializes_transparently_to_raw_string() {
    let token = PatToken::new(LONG_SECRET);
    let json = serde_json::to_string(&token).expect("serialize token");
    assert_eq!(json, format!("\"{LONG_SECRET}\""));
}

#[test]
fn serde_round_trip_preserves_raw_value() {
    let token = BearerToken::new(LONG_SECRET);
    let json = serde_json::to_string(&token).expect("serialize token");
    let back: BearerToken = serde_json::from_str(&json).expect("deserialize token");
    assert_eq!(back.as_str(), LONG_SECRET);
}

#[test]
fn from_str_is_infallible_and_parses_raw() {
    let token = PatToken::from_str(LONG_SECRET).expect("from_str infallible");
    assert_eq!(token.as_str(), LONG_SECRET);
}

#[test]
fn from_owned_and_borrowed_string() {
    let owned: ProxySecret = String::from(SHORT_SECRET).into();
    let borrowed: ProxySecret = SHORT_SECRET.into();
    assert_eq!(owned.as_str(), SHORT_SECRET);
    assert_eq!(borrowed.as_str(), SHORT_SECRET);
}
