//! ID-JAG issuance (`requested_token_type = id-jag`) and the OIDC `id_token`
//! validation it is gated on.
//!
//! `issue_id_jag` only accepts an `id_token`/`jwt` subject and then hands the
//! token to `validate_oidc_subject`, which admits a token only from a trusted
//! issuer explicitly marked `can_issue_id_jag`. Everything up to the JWKS
//! fetch is reachable without a network, so these tests pin the whole
//! rejection surface: subject type, unknown issuer, an issuer that is trusted
//! but not ID-JAG-capable, `typ` allowlist, algorithm, and missing `kid`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use systemprompt_api::routes::oauth::endpoints::token::generation::TokenExchangeRequest;
use systemprompt_api::routes::oauth::endpoints::token::generation::test_api::{
    ACCESS_TOKEN_TYPE, ID_TOKEN_TYPE, JWT_TOKEN_TYPE, issue_id_jag, validate_oidc_subject,
};
use systemprompt_identifiers::ClientId;
use systemprompt_models::Config;
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_test_fixtures::fixture_config;

const IDP: &str = "https://idp.test";

fn jwt(header: &str, payload: &str) -> String {
    format!(
        "{}.{}.c2ln",
        URL_SAFE_NO_PAD.encode(header),
        URL_SAFE_NO_PAD.encode(payload)
    )
}

fn id_token(header: &str) -> String {
    jwt(header, &format!(r#"{{"iss":"{IDP}","sub":"u-1"}}"#))
}

fn trusted_issuer(can_issue_id_jag: bool, typ_allowlist: Vec<String>) -> TrustedIssuer {
    TrustedIssuer {
        issuer: IDP.to_owned(),
        jwks_uri: format!("{IDP}/jwks"),
        audience: "systemprompt".to_owned(),
        typ_allowlist,
        allowed_client_ids: vec![],
        can_issue_id_jag,
    }
}

fn config_with(issuer: TrustedIssuer) -> Config {
    let mut config = fixture_config("postgres://unused/unused");
    config.trusted_issuers = vec![issuer];
    config
}

fn err(result: anyhow::Result<impl Sized>) -> String {
    result.err().expect("expected rejection").to_string()
}

#[tokio::test]
async fn issue_id_jag_rejects_an_access_token_subject() {
    let request = TokenExchangeRequest {
        subject_token: &id_token(r#"{"alg":"RS256","kid":"k1"}"#),
        subject_token_type: ACCESS_TOKEN_TYPE,
        ..TokenExchangeRequest::default()
    };

    let message = err(issue_id_jag(
        &ClientId::new("client-1"),
        &request,
        &config_with(trusted_issuer(true, vec![])),
    )
    .await);

    assert!(
        message.contains("ID-JAG issuance requires an id_token/jwt subject")
            && message.contains(ACCESS_TOKEN_TYPE),
        "{message}"
    );
}

#[tokio::test]
async fn issue_id_jag_admits_both_id_token_and_jwt_subjects_to_oidc_validation() {
    let token = id_token(r#"{"alg":"RS256","kid":"k1"}"#);
    let config = config_with(trusted_issuer(false, vec![]));

    for subject_token_type in [ID_TOKEN_TYPE, JWT_TOKEN_TYPE] {
        let request = TokenExchangeRequest {
            subject_token: &token,
            subject_token_type,
            ..TokenExchangeRequest::default()
        };
        let message = err(issue_id_jag(&ClientId::new("client-1"), &request, &config).await);

        assert!(
            message.contains("is not a trusted ID-JAG issuer"),
            "{subject_token_type} must pass the subject-type gate and fail in OIDC validation: \
             {message}"
        );
    }
}

#[tokio::test]
async fn oidc_rejects_a_malformed_id_token_header() {
    let message =
        err(validate_oidc_subject("not-a-jwt", &config_with(trusted_issuer(true, vec![]))).await);

    assert!(message.contains("malformed id_token header"), "{message}");
}

#[tokio::test]
async fn oidc_rejects_an_unknown_issuer() {
    let token = jwt(
        r#"{"alg":"RS256","kid":"k1"}"#,
        r#"{"iss":"https://attacker.test","sub":"u-1"}"#,
    );

    let message =
        err(validate_oidc_subject(&token, &config_with(trusted_issuer(true, vec![]))).await);

    assert!(
        message.contains("https://attacker.test")
            && message.contains("not a trusted ID-JAG issuer"),
        "{message}"
    );
}

#[tokio::test]
async fn oidc_rejects_a_trusted_issuer_that_may_not_issue_id_jag() {
    let message = err(validate_oidc_subject(
        &id_token(r#"{"alg":"RS256","kid":"k1"}"#),
        &config_with(trusted_issuer(false, vec![])),
    )
    .await);

    assert!(
        message.contains(IDP) && message.contains("not a trusted ID-JAG issuer"),
        "trust alone must not confer ID-JAG issuance: {message}"
    );
}

#[tokio::test]
async fn oidc_rejects_a_typ_outside_the_issuer_allowlist() {
    let config = config_with(trusted_issuer(true, vec!["JWT".to_owned()]));

    let message = err(validate_oidc_subject(
        &id_token(r#"{"alg":"RS256","kid":"k1","typ":"at+jwt"}"#),
        &config,
    )
    .await);

    assert!(message.contains("not in issuer typ_allowlist"), "{message}");
}

#[tokio::test]
async fn oidc_rejects_a_missing_typ_when_the_issuer_declares_an_allowlist() {
    let config = config_with(trusted_issuer(true, vec!["JWT".to_owned()]));

    let message =
        err(validate_oidc_subject(&id_token(r#"{"alg":"RS256","kid":"k1"}"#), &config).await);

    assert!(message.contains("not in issuer typ_allowlist"), "{message}");
}

#[tokio::test]
async fn oidc_accepts_a_typ_inside_the_issuer_allowlist() {
    let config = config_with(trusted_issuer(true, vec!["JWT".to_owned()]));

    let message =
        err(validate_oidc_subject(&id_token(r#"{"alg":"RS256","typ":"JWT"}"#), &config).await);

    assert!(
        message.contains("must carry a kid header"),
        "an allowlisted typ must pass and fail on the next check instead: {message}"
    );
}

#[tokio::test]
async fn oidc_rejects_a_non_rs256_id_token() {
    let message = err(validate_oidc_subject(
        &id_token(r#"{"alg":"HS256","kid":"k1"}"#),
        &config_with(trusted_issuer(true, vec![])),
    )
    .await);

    assert!(message.contains("must be RS256-signed"), "{message}");
}

#[tokio::test]
async fn oidc_rejects_an_id_token_with_no_kid() {
    let message = err(validate_oidc_subject(
        &id_token(r#"{"alg":"RS256"}"#),
        &config_with(trusted_issuer(true, vec![])),
    )
    .await);

    assert!(message.contains("must carry a kid header"), "{message}");
}
