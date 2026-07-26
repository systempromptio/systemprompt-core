//! Subject-token validation for RFC 8693 token exchange.
//!
//! `validate_subject_token` decides which verification path a subject token
//! takes and rejects everything it cannot verify. These tests cover the
//! rejection surface — unsupported token type, malformed token, unknown or
//! untrusted issuer, missing `kid`, wrong algorithm — and the self-issued
//! happy path, which is the one path that needs no network.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use systemprompt_api::routes::oauth::endpoints::token::generation::test_api::validate_subject_token;
use systemprompt_identifiers::UserId;
use systemprompt_models::Config;
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_test_fixtures::{fixture_config, install_test_signing_key, mint_admin_jwt};

const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";
const JWT_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:jwt";

fn config() -> Config {
    fixture_config("postgres://unused/unused")
}

/// A syntactically valid JWT with the given header and payload and a bogus
/// signature — enough to reach every check that runs before verification.
fn unsigned_jwt(header: &str, payload: &str) -> String {
    format!(
        "{}.{}.c2ln",
        URL_SAFE_NO_PAD.encode(header),
        URL_SAFE_NO_PAD.encode(payload)
    )
}

fn err(result: anyhow::Result<impl Sized>) -> String {
    result.err().expect("expected rejection").to_string()
}

#[tokio::test]
async fn rejects_unsupported_subject_token_type() {
    let token = unsigned_jwt(
        r#"{"alg":"RS256","kid":"k1"}"#,
        r#"{"iss":"https://x.test"}"#,
    );

    let message = err(validate_subject_token(
        &token,
        "urn:ietf:params:oauth:token-type:refresh_token",
        &config(),
    )
    .await);

    assert!(message.contains("subject_token_type"), "{message}");
}

#[tokio::test]
async fn accepts_the_three_declared_subject_token_types() {
    let token = unsigned_jwt(
        r#"{"alg":"RS256","kid":"k1"}"#,
        r#"{"iss":"https://x.test"}"#,
    );
    let config = config();

    for token_type in [ACCESS_TOKEN_TYPE, ID_TOKEN_TYPE, JWT_TOKEN_TYPE] {
        let message = err(validate_subject_token(&token, token_type, &config).await);
        assert!(
            !message.contains("subject_token_type"),
            "{token_type} must pass the type check and fail later: {message}"
        );
    }
}

#[tokio::test]
async fn rejects_a_malformed_jwt_header() {
    let message = err(validate_subject_token("not-a-jwt", ACCESS_TOKEN_TYPE, &config()).await);

    assert!(message.contains("malformed JWT header"), "{message}");
}

#[tokio::test]
async fn rejects_an_untrusted_issuer() {
    let token = unsigned_jwt(
        r#"{"alg":"RS256","kid":"k1"}"#,
        r#"{"iss":"https://attacker.test"}"#,
    );

    let message = err(validate_subject_token(&token, ACCESS_TOKEN_TYPE, &config()).await);

    assert!(
        message.contains("is not trusted") && message.contains("https://attacker.test"),
        "{message}"
    );
}

#[tokio::test]
async fn rejects_a_trusted_issuer_token_with_no_kid() {
    let mut config = config();
    config.trusted_issuers = vec![TrustedIssuer {
        issuer: "https://idp.test".to_owned(),
        jwks_uri: "https://idp.test/jwks".to_owned(),
        audience: "systemprompt".to_owned(),
        typ_allowlist: vec![],
        allowed_client_ids: vec![],
        can_issue_id_jag: false,
    }];
    let token = unsigned_jwt(r#"{"alg":"RS256"}"#, r#"{"iss":"https://idp.test"}"#);

    let message = err(validate_subject_token(&token, ACCESS_TOKEN_TYPE, &config).await);

    assert!(message.contains("must carry a kid header"), "{message}");
}

#[tokio::test]
async fn rejects_a_self_issued_token_that_is_not_rs256() {
    let config = config();
    let token = unsigned_jwt(
        r#"{"alg":"HS256","kid":"k1"}"#,
        &format!(r#"{{"iss":"{}"}}"#, config.jwt_issuer),
    );

    let message = err(validate_subject_token(&token, ACCESS_TOKEN_TYPE, &config).await);

    assert!(message.contains("must be RS256-signed"), "{message}");
}

#[tokio::test]
async fn rejects_a_self_issued_token_with_an_unknown_kid() {
    install_test_signing_key();
    let config = config();
    let token = unsigned_jwt(
        r#"{"alg":"RS256","kid":"no-such-key"}"#,
        &format!(r#"{{"iss":"{}"}}"#, config.jwt_issuer),
    );

    let message = err(validate_subject_token(&token, ACCESS_TOKEN_TYPE, &config).await);

    assert!(message.contains("no-such-key"), "{message}");
}

#[tokio::test]
async fn accepts_a_self_issued_token_and_returns_its_scope() {
    let config = config();
    let user = UserId::new("token-exchange-subject");
    let token = mint_admin_jwt(&user, "subject@exchange.invalid", &config.jwt_issuer);

    let identity = validate_subject_token(token.as_str(), ACCESS_TOKEN_TYPE, &config)
        .await
        .expect("a token this issuer signed must validate");

    assert!(
        !identity.scope.is_empty(),
        "an admin token carries its permissions through the exchange"
    );
    assert!(
        identity.prior_act.is_none(),
        "a directly-issued token has no prior delegation chain"
    );
}
