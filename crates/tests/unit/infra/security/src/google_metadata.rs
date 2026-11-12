use std::sync::Arc;
use std::time::Duration;

use systemprompt_identifiers::CredentialId;
use systemprompt_security::credential::{
    AuthScheme, CredentialError, CredentialKind, ProviderCredential, TokenCache, TokenCacheKey,
};
use systemprompt_security::google::{
    DEFAULT_METADATA_TOKEN_URL, MetadataIdentity, metadata_access_token,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN_PATH: &str = "/computeMetadata/v1/instance/service-accounts/default/token";

fn identity(server: &MockServer) -> MetadataIdentity {
    MetadataIdentity::at(format!("{}{TOKEN_PATH}", server.uri()))
}

fn token(value: &str, expires_in: u64) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "access_token": value,
        "expires_in": expires_in,
        "token_type": "Bearer"
    }))
}

#[test]
fn the_ambient_identity_is_the_metadata_server() {
    assert_eq!(
        MetadataIdentity::ambient().token_url(),
        DEFAULT_METADATA_TOKEN_URL
    );
    assert_eq!(MetadataIdentity::default(), MetadataIdentity::ambient());
}

#[tokio::test]
async fn a_long_lived_token_is_minted_once_and_reused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .and(header("Metadata-Flavor", "Google"))
        .respond_with(token("token-1", 3600))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = TokenCache::new();
    let identity = identity(&server);
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "token-1"
    );
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "token-1"
    );
}

#[tokio::test]
async fn a_token_inside_the_expiry_skew_is_minted_again() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("short", 90))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("fresh", 3600))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = TokenCache::new();
    let identity = identity(&server);
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "short"
    );
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "fresh"
    );
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "fresh"
    );
}

#[tokio::test]
async fn concurrent_callers_share_one_mint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("shared", 3600).set_delay(Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = Arc::new(TokenCache::new());
    let identity = identity(&server);
    let calls: Vec<_> = (0..8)
        .map(|_| {
            let tokens = Arc::clone(&tokens);
            let identity = identity.clone();
            tokio::spawn(async move { metadata_access_token(&tokens, &identity).await })
        })
        .collect();
    for call in calls {
        assert_eq!(call.await.unwrap().unwrap(), "shared");
    }
}

#[tokio::test]
async fn a_refused_mint_is_an_error_and_is_not_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(ResponseTemplate::new(403).set_body_string("no binding"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("after", 3600))
        .mount(&server)
        .await;
    let tokens = TokenCache::new();
    let identity = identity(&server);
    let err = metadata_access_token(&tokens, &identity).await.unwrap_err();
    assert!(
        matches!(err, CredentialError::Rejected { ref body, .. } if body == "no binding"),
        "{err:?}"
    );
    assert_eq!(
        metadata_access_token(&tokens, &identity).await.unwrap(),
        "after"
    );
}

#[tokio::test]
async fn two_credentials_on_one_identity_share_its_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(TOKEN_PATH))
        .respond_with(token("ambient", 3600))
        .expect(1)
        .mount(&server)
        .await;
    let tokens = TokenCache::new();
    let gemini = ProviderCredential::google_workload_identity("acme".into(), identity(&server));
    let claude = ProviderCredential::google_workload_identity("acme".into(), identity(&server));
    let first = gemini
        .bearer(
            &tokens,
            &TokenCacheKey::platform(&CredentialId::new("vertex_a")),
        )
        .await
        .unwrap();
    let second = claude
        .bearer(
            &tokens,
            &TokenCacheKey::platform(&CredentialId::new("vertex_b")),
        )
        .await
        .unwrap();
    assert_eq!(first.scheme, AuthScheme::Bearer);
    assert_eq!(first.value, "ambient");
    assert_eq!(second.value, "ambient");
    assert_eq!(
        metadata_access_token(&tokens, &identity(&server))
            .await
            .unwrap(),
        "ambient"
    );
}

#[test]
fn a_workload_identity_scopes_its_declared_project() {
    let credential = ProviderCredential::google_workload_identity(
        "acme-prod".into(),
        MetadataIdentity::ambient(),
    );
    assert_eq!(credential.kind(), CredentialKind::GoogleWorkloadIdentity);
    assert!(credential.kind().is_google_identity());
    assert_eq!(credential.kind().as_str(), "google_workload_identity");
    assert_eq!(credential.scope().project.as_deref(), Some("acme-prod"));
    assert_eq!(credential.scope().principal, None);
    assert_eq!(
        credential
            .fill_endpoint(
                "https://aiplatform.googleapis.com/v1/projects/{project}/locations/global"
            )
            .unwrap(),
        "https://aiplatform.googleapis.com/v1/projects/acme-prod/locations/global"
    );
    assert!(!CredentialKind::ApiKey.is_google_identity());
}
