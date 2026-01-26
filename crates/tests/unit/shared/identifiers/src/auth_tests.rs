//! Unit tests for JwtToken type.

use std::collections::HashSet;
use systemprompt_identifiers::JwtToken;

#[test]
fn test_jwt_token_new_from_str() {
    let token = JwtToken::new("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test");
    assert_eq!(token.as_str(), "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.test");
}

#[test]
fn test_jwt_token_new_from_string() {
    let token = JwtToken::new(String::from("jwt-token-string"));
    assert_eq!(token.as_str(), "jwt-token-string");
}

#[test]
fn test_jwt_token_display() {
    let token = JwtToken::new("display-token");
    assert_eq!(format!("{}", token), "********");
}

#[test]
fn test_jwt_token_from_string() {
    let token: JwtToken = String::from("from-string-token").into();
    assert_eq!(token.as_str(), "from-string-token");
}

#[test]
fn test_jwt_token_from_str() {
    let token: JwtToken = "from-str-token".into();
    assert_eq!(token.as_str(), "from-str-token");
}

#[test]
fn test_jwt_token_as_ref() {
    let token = JwtToken::new("as-ref-token");
    let s: &str = token.as_ref();
    assert_eq!(s, "as-ref-token");
}

#[test]
fn test_jwt_token_clone() {
    let token1 = JwtToken::new("clone-token");
    let token2 = token1.clone();
    assert_eq!(token1, token2);
}

#[test]
fn test_jwt_token_equality() {
    let token1 = JwtToken::new("equal-token");
    let token2 = JwtToken::new("equal-token");
    let token3 = JwtToken::new("different-token");

    assert_eq!(token1, token2);
    assert_ne!(token1, token3);
}

#[test]
fn test_jwt_token_hash() {
    let token1 = JwtToken::new("hash-token");
    let token2 = JwtToken::new("hash-token");

    let mut set = HashSet::new();
    set.insert(token1.clone());

    assert!(set.contains(&token2));
}

#[test]
fn test_jwt_token_serialize_json() {
    let token = JwtToken::new("serialize-token");
    let json = serde_json::to_string(&token).unwrap();
    assert_eq!(json, "\"serialize-token\"");
}

#[test]
fn test_jwt_token_deserialize_json() {
    let token: JwtToken = serde_json::from_str("\"deserialize-token\"").unwrap();
    assert_eq!(token.as_str(), "deserialize-token");
}

#[test]
fn test_jwt_token_debug() {
    let token = JwtToken::new("debug-token");
    let debug_str = format!("{:?}", token);
    assert!(debug_str.contains("JwtToken"));
    assert!(debug_str.contains("debug-token"));
}

#[test]
fn test_jwt_token_empty_string() {
    let token = JwtToken::new("");
    assert_eq!(token.as_str(), "");
}

#[test]
fn test_jwt_token_long_token() {
    let long_token = "a".repeat(1000);
    let token = JwtToken::new(&long_token);
    assert_eq!(token.as_str(), long_token);
}

#[test]
fn test_jwt_token_realistic_format() {
    // A realistic JWT format (header.payload.signature)
    let realistic_jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
        eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.\
        SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
    let token = JwtToken::new(realistic_jwt);
    assert_eq!(token.as_str(), realistic_jwt);
}
