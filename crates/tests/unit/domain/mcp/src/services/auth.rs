//! Unit tests for `validate_jwt_token` error paths.
//!
//! Happy paths require a signing-key authority to be loaded, which is global
//! state owned by `systemprompt-security` and not available in unit tests.

use systemprompt_mcp::services::auth::validate_jwt_token;
use systemprompt_models::auth::JwtAudience;

#[test]
fn rejects_malformed_token() {
    let err = validate_jwt_token("not-a-jwt", "issuer", &[JwtAudience::Mcp]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("JWT header decode failed") || msg.contains("header"),
        "unexpected: {msg}"
    );
}

#[test]
fn rejects_empty_token() {
    validate_jwt_token("", "iss", &[JwtAudience::Mcp]).unwrap_err();
}

#[test]
fn rejects_non_rs256_token() {
    // HS256 header: {"alg":"HS256","typ":"JWT"} → eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
    // body and signature are arbitrary placeholders.
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.signature";
    let err = validate_jwt_token(token, "issuer", &[JwtAudience::Mcp]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("RS256") || msg.contains("header") || msg.contains("kid"),
        "unexpected: {msg}"
    );
}

#[test]
fn rejects_rs256_token_without_kid() {
    // RS256 header without `kid`.
    let token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.e30.signature";
    let err = validate_jwt_token(token, "issuer", &[JwtAudience::Mcp]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("kid") || msg.contains("signing key"),
        "unexpected: {msg}"
    );
}

#[test]
fn rejects_rs256_token_with_unknown_kid() {
    // RS256 header with kid="nope". {"alg":"RS256","kid":"nope","typ":"JWT"}
    let token =
        "eyJhbGciOiJSUzI1NiIsImtpZCI6Im5vcGUiLCJ0eXAiOiJKV1QifQ.e30.signature";
    let err = validate_jwt_token(token, "issuer", &[JwtAudience::Mcp]).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("kid") || msg.contains("signing key") || msg.contains("unknown"),
        "unexpected: {msg}"
    );
}
