//! Additional `validate_jwt_token` error-path coverage.
//!
//! `jwt_alg.rs` already covers the algorithm-mismatch branch. This
//! module exercises the remaining deterministic early-exit branches
//! that fire before signature verification: malformed input, RS256
//! token without a `kid` header, and RS256 token whose `kid` is not
//! registered with the authority keyring.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use systemprompt_models::auth::JwtAudience;
use systemprompt_oauth::{OauthError, validate_jwt_token};

fn forge_jwt(header_json: &str) -> String {
    let header = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
    let payload = URL_SAFE_NO_PAD.encode(b"{}");
    let sig = URL_SAFE_NO_PAD.encode(b"signature-placeholder");
    format!("{header}.{payload}.{sig}")
}

#[test]
fn empty_token_is_rejected() {
    let err = validate_jwt_token("", "issuer", &[JwtAudience::Api])
        .expect_err("empty input must fail header decode");

    assert!(matches!(err, OauthError::TokenInvalid(_)));
}

#[test]
fn garbage_token_is_rejected() {
    let err = validate_jwt_token("not-a-jwt", "issuer", &[JwtAudience::Api])
        .expect_err("garbage input must fail header decode");

    assert!(matches!(err, OauthError::TokenInvalid(_)));
}

#[test]
fn two_segment_token_is_rejected() {
    let err = validate_jwt_token("aaa.bbb", "issuer", &[JwtAudience::Api])
        .expect_err("two-segment input must fail header decode");

    assert!(matches!(err, OauthError::TokenInvalid(_)));
}

#[test]
fn rs256_without_kid_yields_token_missing_kid() {
    let token = forge_jwt(r#"{"alg":"RS256","typ":"JWT"}"#);

    let err = validate_jwt_token(&token, "issuer", &[JwtAudience::Api])
        .expect_err("missing kid must be rejected");

    assert!(matches!(err, OauthError::TokenMissingKid));
}

#[test]
fn rs256_with_unknown_kid_yields_unknown_kid_or_lookup_failure() {
    let token = forge_jwt(r#"{"alg":"RS256","kid":"unknown-key-id","typ":"JWT"}"#);

    let err = validate_jwt_token(&token, "issuer", &[JwtAudience::Api])
        .expect_err("unknown kid must be rejected");

    match err {
        OauthError::TokenUnknownKid { kid } => {
            assert_eq!(kid, "unknown-key-id");
        },
        OauthError::TokenInvalid(msg) => {
            assert!(
                msg.contains("signing key lookup") || msg.contains("key"),
                "unexpected token-invalid message: {msg}"
            );
        },
        other => panic!("expected TokenUnknownKid or TokenInvalid, got {other:?}"),
    }
}
