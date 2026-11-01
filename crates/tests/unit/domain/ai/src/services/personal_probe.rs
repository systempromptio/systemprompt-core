use systemprompt_ai::{PersonalKeyError, personal_credential, verify_personal_key};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::catalog::{CatalogFixture, model, personal, priced};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{body_string_contains, method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn catalog(base: &str, deployed: bool) -> ResolvedCatalog {
    let fixture = CatalogFixture::new()
        .credential("go", personal("Go"))
        .provider("go", ProviderConfig::new("go"))
        .endpoint(
            "go",
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, format!("{base}/chat/v1")),
        )
        .model(
            "go-chat",
            model(&[ClientFamily::OpenAi], 128_000, priced(1.0, 2.0, 0.1)),
        );
    if deployed {
        fixture.deploy("go-chat", ["go/chat"]).build()
    } else {
        fixture
            .endpoint(
                "go",
                "other",
                EndpointConfig::new(WireProtocol::OpenAiChat, format!("{base}/other/v1")),
            )
            .deploy("go-chat", ["go/other"])
            .build()
    }
}

fn tokens() -> std::sync::Arc<dyn systemprompt_traits::ProviderTokenCache> {
    std::sync::Arc::new(systemprompt_security::credential::TokenCache::new())
}

async fn verify(catalog: &ResolvedCatalog) -> Result<(), PersonalKeyError> {
    let credential = personal_credential(catalog, "go").expect("personal");
    verify_personal_key(
        &tokens(),
        catalog,
        credential,
        "sk-go",
        &UserId::new("member-1!"),
    )
    .await
}

async fn mount_models(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path("/chat/v1/models"))
        .respond_with(ResponseTemplate::new(status))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_probe(server: &MockServer, status: u16) {
    Mock::given(method("POST"))
        .and(path_regex("^/chat/v1/.+"))
        .and(body_string_contains("\"max_tokens\":1"))
        .respond_with(ResponseTemplate::new(status))
        .expect(1)
        .mount(server)
        .await;
}

async fn probed(models: u16, completion: u16) -> Result<(), PersonalKeyError> {
    let server = MockServer::start().await;
    mount_models(&server, models).await;
    mount_probe(&server, completion).await;
    let result = verify(&catalog(&server.uri(), true)).await;
    server.verify().await;
    result
}

#[tokio::test]
async fn a_missing_models_listing_falls_back_to_a_one_token_completion() {
    probed(404, 200)
        .await
        .expect("the completion proves the key");
}

#[tokio::test]
async fn a_completion_refused_past_authentication_still_proves_the_key() {
    for status in [400, 422, 429] {
        probed(405, status).await.expect("past authentication");
    }
}

#[tokio::test]
async fn a_completion_answered_401_rejects_the_key() {
    let err = probed(404, 401).await.unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Rejected(ref l) if l == "Go"),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_completion_answered_500_is_unverified_with_its_status() {
    let err = probed(404, 500).await.unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Unverified { ref status, .. } if status == "500"),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_models_listing_answered_503_is_unverified() {
    let server = MockServer::start().await;
    mount_models(&server, 503).await;
    let err = verify(&catalog(&server.uri(), true)).await.unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Unverified { ref status, .. } if status == "503"),
        "{err:?}"
    );
}

#[tokio::test]
async fn a_models_listing_answered_403_rejects_the_key() {
    let server = MockServer::start().await;
    mount_models(&server, 403).await;
    let err = verify(&catalog(&server.uri(), true)).await.unwrap_err();
    assert!(matches!(err, PersonalKeyError::Rejected(_)), "{err:?}");
}

#[tokio::test]
async fn an_endpoint_with_no_deployed_model_cannot_be_probed() {
    let server = MockServer::start().await;
    mount_models(&server, 404).await;
    let err = verify(&catalog(&server.uri(), false)).await.unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Unverified { ref status, .. } if status == "no model to probe"),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_unreachable_endpoint_is_unverified() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let err = verify(&catalog(&base, true)).await.unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Unverified { ref status, .. } if status == "no response"),
        "{err:?}"
    );
}
