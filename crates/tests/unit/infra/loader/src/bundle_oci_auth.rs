//! The Bearer challenge parser, the credential shape, and the token exchange.
//!
//! An unrecognised challenge must fail rather than silently downgrade to an
//! anonymous request, and a token endpoint that answers without a token is a
//! failed authentication, not an empty bearer.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use serde_json::json;
use systemprompt_loader::bundle::source::oci::auth::{
    BearerChallenge, apply_credential, fetch_token, parse_challenge,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn header_of(builder: reqwest::RequestBuilder) -> Option<String> {
    builder
        .build()
        .expect("request")
        .headers()
        .get(reqwest::header::AUTHORIZATION)
        .map(|v| v.to_str().expect("ascii").to_owned())
}

#[test]
fn a_full_challenge_parses_into_realm_service_and_scope() {
    let parsed = parse_challenge(
        "Bearer realm=\"https://auth.example/token\",service=\"reg\",scope=\"repository:o/b:pull\"",
    )
    .expect("challenge");

    assert_eq!(
        parsed,
        BearerChallenge {
            realm: "https://auth.example/token".to_owned(),
            service: Some("reg".to_owned()),
            scope: Some("repository:o/b:pull".to_owned()),
        }
    );
}

#[test]
fn a_lowercase_scheme_parses_and_unknown_parameters_are_ignored() {
    let parsed = parse_challenge("bearer realm=\"https://auth.example/t\",error=\"insufficient\"")
        .expect("challenge");

    assert_eq!(parsed.realm, "https://auth.example/t");
    assert_eq!(parsed.service, None);
    assert_eq!(parsed.scope, None);
}

#[test]
fn a_challenge_without_a_realm_is_not_a_challenge() {
    assert_eq!(parse_challenge("Bearer service=\"reg\""), None);
}

#[test]
fn a_basic_challenge_is_not_treated_as_bearer() {
    assert_eq!(parse_challenge("Basic realm=\"reg\""), None);
}

#[test]
fn a_challenge_parameter_without_a_value_is_rejected() {
    assert_eq!(parse_challenge("Bearer realm"), None);
}

#[test]
fn a_secret_without_a_colon_is_sent_as_a_bearer_token() {
    let sent = header_of(apply_credential(
        client().get("http://localhost/x"),
        Some("tok"),
    ));

    assert_eq!(sent.as_deref(), Some("Bearer tok"));
}

#[test]
fn a_user_colon_token_secret_is_sent_as_http_basic() {
    let sent = header_of(apply_credential(
        client().get("http://localhost/x"),
        Some("user:token"),
    ));

    assert_eq!(sent.as_deref(), Some("Basic dXNlcjp0b2tlbg=="));
}

#[test]
fn no_secret_leaves_the_request_anonymous() {
    assert_eq!(
        header_of(apply_credential(client().get("http://localhost/x"), None)),
        None
    );
}

#[tokio::test]
async fn the_token_endpoint_receives_the_service_and_scope_from_the_challenge() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .and(wiremock::matchers::query_param("service", "reg"))
        .and(wiremock::matchers::query_param(
            "scope",
            "repository:o/b:pull",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"token": "issued"})))
        .mount(&server)
        .await;

    let token = fetch_token(
        &client(),
        &BearerChallenge {
            realm: format!("{}/token", server.uri()),
            service: Some("reg".to_owned()),
            scope: Some("repository:o/b:pull".to_owned()),
        },
        None,
        "base",
    )
    .await
    .expect("token");

    assert_eq!(token, "issued");
}

#[tokio::test]
async fn an_access_token_field_is_accepted_when_token_is_absent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"access_token": "alt"})))
        .mount(&server)
        .await;

    let token = fetch_token(
        &client(),
        &BearerChallenge {
            realm: format!("{}/token", server.uri()),
            service: None,
            scope: None,
        },
        Some("user:token"),
        "base",
    )
    .await
    .expect("token");

    assert_eq!(token, "alt");
}

#[tokio::test]
async fn a_token_response_carrying_no_token_is_an_auth_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let err = fetch_token(
        &client(),
        &BearerChallenge {
            realm: format!("{}/token", server.uri()),
            service: None,
            scope: None,
        },
        None,
        "base",
    )
    .await
    .expect_err("an empty body is not a token");

    assert!(
        err.to_string().contains("base"),
        "the failure names the source: {err}"
    );
}

#[tokio::test]
async fn a_rejected_token_request_is_an_auth_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let err = fetch_token(
        &client(),
        &BearerChallenge {
            realm: format!("{}/token", server.uri()),
            service: None,
            scope: None,
        },
        Some("bad"),
        "base",
    )
    .await
    .expect_err("a refused exchange is not a token");

    assert!(
        !err.to_string().contains("bad"),
        "the credential never appears in the error: {err}"
    );
}

#[tokio::test]
async fn a_token_body_that_does_not_parse_names_the_source() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;

    let err = fetch_token(
        &client(),
        &BearerChallenge {
            realm: format!("{}/token", server.uri()),
            service: None,
            scope: None,
        },
        None,
        "base",
    )
    .await
    .expect_err("a non-JSON body is not a token");

    assert!(
        err.to_string().contains("token response"),
        "the error names the stage that failed: {err}"
    );
}

#[tokio::test]
async fn an_unreachable_realm_is_a_fetch_failure() {
    let err = fetch_token(
        &client(),
        &BearerChallenge {
            realm: "http://127.0.0.1:1/token".to_owned(),
            service: None,
            scope: None,
        },
        None,
        "base",
    )
    .await
    .expect_err("nothing is listening");

    assert!(
        err.to_string().contains("base"),
        "the error names the source: {err}"
    );
}
