//! The provider-credential model: parsing, scope, endpoint filling, and the
//! single-flight token cache.

use std::sync::Arc;

use serde_json::json;
use systemprompt_security::credential::cache::clamp_ttl;
use systemprompt_security::credential::{
    AuthScheme, CredentialKind, CredentialScope, PROJECT_PLACEHOLDER, ProviderCredential,
    REGION_PLACEHOLDER, fill_endpoint,
};
use systemprompt_test_fixtures::keys::test_key;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn service_account(token_uri: &str) -> String {
    json!({
        "type": "service_account",
        "project_id": "fixture-project",
        "client_email": "fixture@example.invalid",
        "private_key": test_key(1).to_pkcs8_pem().unwrap(),
        "token_uri": token_uri,
    })
    .to_string()
}

fn vertex_endpoint() -> String {
    format!(
        "https://us-central1-aiplatform.googleapis.com/v1/projects/{PROJECT_PLACEHOLDER}/locations/us-central1/publishers/google"
    )
}

#[test]
fn the_secret_itself_decides_which_credential_it_is() {
    for opaque in [
        "plain-api-key",
        "not json",
        "null",
        "[]",
        "{}",
        r#"{"type":"user"}"#,
    ] {
        let credential = ProviderCredential::parse(opaque).unwrap();
        assert_eq!(credential.kind(), CredentialKind::ApiKey, "{opaque}");
    }
    let credential =
        ProviderCredential::parse(&service_account("https://example.invalid")).unwrap();
    assert_eq!(credential.kind(), CredentialKind::GoogleServiceAccount);
}

#[test]
fn a_declared_service_account_that_does_not_parse_is_an_error_not_an_api_key() {
    let error = ProviderCredential::parse(r#"{"type":"service_account"}"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("malformed"), "{error}");
}

#[tokio::test]
async fn an_api_key_names_nothing_and_is_sent_verbatim() {
    let credential = ProviderCredential::parse("plain-api-key").unwrap();
    assert_eq!(credential.scope(), CredentialScope::empty());
    let header = credential.bearer("any-secret").await.unwrap();
    assert_eq!(header.scheme, AuthScheme::ApiKey);
    assert!(!header.is_bearer());
    assert_eq!(header.value, "plain-api-key");
}

#[test]
fn a_service_account_names_its_project_and_its_principal_but_no_region() {
    let credential =
        ProviderCredential::parse(&service_account("https://example.invalid")).unwrap();
    let scope = credential.scope();
    assert_eq!(scope.project.as_deref(), Some("fixture-project"));
    assert_eq!(scope.principal.as_deref(), Some("fixture@example.invalid"));
    assert_eq!(scope.region, None);
}

#[test]
fn a_credential_never_prints_its_secret() {
    for secret in ["plain-api-key", &service_account("https://example.invalid")] {
        let rendered = format!("{:?}", ProviderCredential::parse(secret).unwrap());
        assert!(rendered.contains("redacted"), "{rendered}");
        assert!(!rendered.contains("plain-api-key"), "{rendered}");
        assert!(!rendered.contains("PRIVATE KEY"), "{rendered}");
    }
}

#[test]
fn every_placeholder_the_scope_can_supply_is_filled() {
    let scope = CredentialScope {
        project: Some("acme-123".to_owned()),
        region: Some("europe-west4".to_owned()),
        principal: None,
    };
    let template =
        format!("https://{REGION_PLACEHOLDER}-example.invalid/v1/projects/{PROJECT_PLACEHOLDER}/x");
    assert_eq!(
        fill_endpoint(&template, &scope).unwrap(),
        "https://europe-west4-example.invalid/v1/projects/acme-123/x"
    );
}

#[test]
fn an_endpoint_with_no_placeholder_is_untouched() {
    assert_eq!(
        fill_endpoint("https://api.anthropic.com/v1", &CredentialScope::empty()).unwrap(),
        "https://api.anthropic.com/v1"
    );
}

// Why: guessing a project id would send a customer's inference to a cloud
// account the operator never named, so the refusal is the feature.
#[test]
fn an_endpoint_asking_for_a_coordinate_the_credential_lacks_is_refused() {
    let endpoint = vertex_endpoint();
    let refused = fill_endpoint(&endpoint, &CredentialScope::empty())
        .unwrap_err()
        .to_string();
    assert!(refused.contains("project_id"), "{refused}");
    assert!(refused.contains(&endpoint), "{refused}");

    let empty = CredentialScope {
        project: Some(String::new()),
        ..CredentialScope::empty()
    };
    assert!(
        fill_endpoint(&endpoint, &empty).is_err(),
        "an empty project is no project"
    );

    let region_only = format!("https://{REGION_PLACEHOLDER}.example.invalid/v1");
    let google = ProviderCredential::parse(&service_account("https://example.invalid")).unwrap();
    assert!(
        fill_endpoint(&region_only, &google.scope()).is_err(),
        "a service account carries no region"
    );
}

#[test]
fn a_declared_lifetime_is_clamped_into_the_range_we_are_willing_to_cache() {
    assert_eq!(clamp_ttl(None).as_secs(), 3600);
    assert_eq!(clamp_ttl(Some(0)).as_secs(), 3600);
    assert_eq!(clamp_ttl(Some(5)).as_secs(), 60);
    assert_eq!(clamp_ttl(Some(900)).as_secs(), 900);
    assert_eq!(clamp_ttl(Some(86_400)).as_secs(), 3600);
}

async fn token_endpoint(server: &MockServer, response: ResponseTemplate, calls: u64) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(response)
        .expect(calls)
        .mount(server)
        .await;
}

// Why: without single-flight, a cold instance taking its first N concurrent
// requests mints N tokens — a burst identity providers rate-limit, leaving
// N-1 tokens outstanding.
#[tokio::test]
async fn concurrent_first_calls_mint_exactly_one_token() {
    let server = MockServer::start().await;
    token_endpoint(
        &server,
        ResponseTemplate::new(200)
            .set_body_json(json!({"access_token":"single","expires_in":3600}))
            .set_delay(std::time::Duration::from_millis(150)),
        1,
    )
    .await;
    let credential = Arc::new(
        ProviderCredential::parse(&service_account(&format!("{}/token", server.uri()))).unwrap(),
    );
    let name = uuid::Uuid::new_v4().to_string();

    let one = || {
        let credential = Arc::clone(&credential);
        let name = name.clone();
        async move { credential.bearer(&name).await.unwrap().value }
    };
    let (a, b, c, d) = tokio::join!(one(), one(), one(), one());
    for value in [a, b, c, d] {
        assert_eq!(value, "single");
    }
    server.verify().await;
}

// Why: a connect failure is the identity provider being briefly unavailable,
// which one retry fixes; a 4xx is a credential an operator has to change.
#[tokio::test]
async fn a_transient_server_error_is_retried_once_and_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(503).set_body_string("try later"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    token_endpoint(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"access_token":"recovered"})),
        1,
    )
    .await;
    let credential =
        ProviderCredential::parse(&service_account(&format!("{}/token", server.uri()))).unwrap();
    let header = credential
        .bearer(&uuid::Uuid::new_v4().to_string())
        .await
        .unwrap();
    assert_eq!(header.value, "recovered");
    assert!(header.is_bearer());
    server.verify().await;
}

#[tokio::test]
async fn a_refused_credential_is_not_retried() {
    let server = MockServer::start().await;
    token_endpoint(
        &server,
        ResponseTemplate::new(403).set_body_string(" permission denied "),
        1,
    )
    .await;
    let credential =
        ProviderCredential::parse(&service_account(&format!("{}/token", server.uri()))).unwrap();
    let error = credential
        .bearer(&uuid::Uuid::new_v4().to_string())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("403 Forbidden: permission denied"),
        "{error}"
    );
    server.verify().await;
}
