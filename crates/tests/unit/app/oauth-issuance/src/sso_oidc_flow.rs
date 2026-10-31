use serde_json::json;
use systemprompt_oauth_issuance::sso::oidc::OidcProvider;
use systemprompt_traits::sso::{
    CookieSameSite, SsoError, SsoProtocol, SsoProvider, SsoReturn, SsoReturnBinding, SsoStart,
    SsoStateContext,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::sso_oidc::{MapSecrets, config, keycloak};

fn provider(issuer: &str) -> OidcProvider {
    OidcProvider::new(
        config(issuer),
        MapSecrets::with_client_secret(),
        "https://console.example.com",
    )
    .expect("provider")
}

fn callback(params: &[(&str, &str)]) -> SsoReturn {
    SsoReturn {
        binding: SsoReturnBinding::Query,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

fn state() -> Option<SsoStateContext> {
    Some(SsoStateContext {
        request_id: None,
        nonce: Some("n".to_owned()),
        pkce_verifier: Some("v".to_owned()),
    })
}

fn assertion(result: Result<impl std::fmt::Debug, SsoError>) -> String {
    match result {
        Err(SsoError::InvalidAssertion(reason)) => reason,
        other => panic!("expected an invalid assertion, got {other:?}"),
    }
}

async fn token_answers(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/realms/acme/protocol/openid-connect/token"))
        .respond_with(response)
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_provider_describes_itself_from_its_config() {
    let (_server, issuer) = keycloak().await;
    let provider = provider(&format!("{issuer}/"));

    assert_eq!(provider.id(), "oidc");
    assert_eq!(provider.issuer(), issuer);
    assert_eq!(provider.protocol(), SsoProtocol::Oidc);
    assert_eq!(provider.paths().callback, "/api/v1/core/sso/oidc/callback");
    assert_eq!(provider.paths().start, "/api/v1/core/sso/oidc/start");
    assert_eq!(provider.login_path(), "/admin/login");
    assert_eq!(provider.default_return_to(), "/admin");
    assert_eq!(provider.return_cookie_same_site(), CookieSameSite::Lax);
    assert!(provider.policy().deny_without_group);
    assert!(format!("{provider:?}").contains(&issuer));
}

#[tokio::test]
async fn callbacks_without_a_code_or_with_partial_state_are_refused() {
    let (_server, issuer) = keycloak().await;
    let provider = provider(&issuer);

    assert_eq!(
        assertion(provider.finish(callback(&[("state", "s")]), state()).await),
        "missing code"
    );
    let partial = Some(SsoStateContext {
        request_id: None,
        nonce: Some("n".to_owned()),
        pkce_verifier: None,
    });
    assert!(matches!(
        provider.finish(callback(&[("code", "c")]), partial).await,
        Err(SsoError::StateLost)
    ));
}

#[tokio::test]
async fn an_authorization_response_from_another_issuer_is_refused() {
    let (_server, issuer) = keycloak().await;
    let provider = provider(&issuer);

    let outcome = provider
        .finish(
            callback(&[("code", "c"), ("iss", "https://evil.example.com")]),
            state(),
        )
        .await;

    assert_eq!(assertion(outcome), "iss");
}

#[tokio::test]
async fn a_refused_code_is_an_invalid_assertion() {
    let (server, issuer) = keycloak().await;
    token_answers(
        &server,
        ResponseTemplate::new(400).set_body_json(json!({ "error": "invalid_grant" })),
    )
    .await;

    let outcome = provider(&issuer)
        .finish(callback(&[("code", "c"), ("iss", &issuer)]), state())
        .await;

    assert_eq!(assertion(outcome), "code refused");
}

#[tokio::test]
async fn a_token_response_without_an_id_token_is_refused() {
    let (server, issuer) = keycloak().await;
    token_answers(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({ "token_type": "Bearer" })),
    )
    .await;

    let outcome = provider(&issuer)
        .finish(callback(&[("code", "c")]), state())
        .await;

    assert_eq!(assertion(outcome), "no id_token");
}

#[tokio::test]
async fn a_failing_token_endpoint_is_an_internal_error() {
    let (server, issuer) = keycloak().await;
    token_answers(&server, ResponseTemplate::new(502)).await;

    let outcome = provider(&issuer)
        .finish(callback(&[("code", "c")]), state())
        .await;

    assert!(matches!(outcome, Err(SsoError::Internal(_))), "{outcome:?}");
}

#[tokio::test]
async fn an_unreachable_discovery_document_fails_the_start() {
    let server = MockServer::start().await;
    let issuer = format!("{}/realms/acme", server.uri());

    let outcome = provider(&issuer)
        .start(SsoStart {
            return_to: "/admin",
            callback_url: "https://console.example.com/api/v1/core/sso/oidc/callback",
        })
        .await;

    assert!(matches!(outcome, Err(SsoError::Internal(_))), "{outcome:?}");
}
