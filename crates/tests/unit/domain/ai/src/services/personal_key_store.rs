//! Storing a member's key.
//!
//! Pins that a blank or oversized key is refused before any upstream is
//! called, that a verified key is sealed and stored under its credential, and
//! that a credential's label falls back to its id.

use std::sync::Arc;

use systemprompt_ai::repository::UserProviderCredentialRepository;
use systemprompt_ai::{PersonalKeyError, PersonalKeyRequest, credential_label, store_personal_key};
use systemprompt_identifiers::{CredentialId, UserId};
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_models::providers::ClientFamily;
use systemprompt_security::at_rest::AtRestCipher;
use systemprompt_security::credential::TokenCache;
use systemprompt_test_fixtures::catalog::{CatalogFixture, model, personal, priced};
use systemprompt_test_fixtures::{
    ensure_test_secrets_bootstrap, seed_user_row, test_db_pool, unique_user_id,
};
use systemprompt_traits::ProviderTokenCache;
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn single_endpoint(base: &str, deployed: bool) -> ResolvedCatalog {
    let fixture = CatalogFixture::new()
        .credential("go", personal("Go"))
        .provider(
            "go",
            ProviderConfig {
                forward_headers: vec!["x-opencode-session".to_owned()],
                ..ProviderConfig::new("go")
            },
        )
        .endpoint(
            "go",
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, format!("{base}/v1")),
        );
    if deployed {
        fixture
            .model(
                "go-chat",
                model(&[ClientFamily::OpenAi], 128_000, priced(1.0, 2.0, 0.1)),
            )
            .deploy("go-chat", ["go/chat"])
            .build()
    } else {
        fixture.build()
    }
}

fn tokens() -> Arc<dyn ProviderTokenCache> {
    Arc::new(TokenCache::new())
}

async fn models_status(server: &MockServer, status: u16) {
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

#[test]
fn the_label_falls_back_to_the_credential_id() {
    let catalog = CatalogFixture::new()
        .credential(
            "bare",
            systemprompt_manifest::services::catalog::CredentialEntry::personal(None),
        )
        .provider("bare", ProviderConfig::new("bare"))
        .endpoint(
            "bare",
            "api",
            EndpointConfig::new(WireProtocol::OpenAiChat, "https://bare.example.test/v1"),
        )
        .build();
    let credential = catalog.credential("bare").expect("declared");
    assert_eq!(credential_label(credential), "bare");
    assert_eq!(
        PersonalKeyError::NotPersonalCredential("x".to_owned()).to_string(),
        "credential 'x' does not take a personal key"
    );
}

async fn repo_and_user(prefix: &str) -> (UserProviderCredentialRepository, UserId) {
    let secrets = ensure_test_secrets_bootstrap();
    let cipher = AtRestCipher::from_secrets(&secrets.get()).expect("cipher");
    let pool = test_db_pool().await;
    let user = unique_user_id(prefix);
    seed_user_row(&pool, &user, &format!("{}@personal.invalid", user.as_str()))
        .await
        .expect("seed user");
    (UserProviderCredentialRepository::new(&pool, cipher), user)
}

#[tokio::test]
async fn a_verified_key_is_sealed_and_stored_under_its_credential() {
    let server = MockServer::start().await;
    models_status(&server, 200).await;
    let catalog = single_endpoint(&server.uri(), true);
    let (repo, user) = repo_and_user("store-key").await;

    let meta = store_personal_key(
        &tokens(),
        &repo,
        &catalog,
        PersonalKeyRequest {
            user_id: &user,
            credential: "go",
            key: "  sk-go-stored  ",
            label: Some("   "),
        },
    )
    .await
    .expect("stored");
    assert_eq!(meta.credential, "go");
    assert_eq!(meta.label, None, "a blank label is not stored");
    assert_eq!(
        repo.find_key(&user, &CredentialId::new("go"))
            .await
            .expect("read")
            .as_deref(),
        Some("sk-go-stored")
    );
}

#[tokio::test]
async fn a_blank_or_oversized_key_is_refused_without_calling_upstream() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let catalog = single_endpoint(&server.uri(), true);
    let (repo, user) = repo_and_user("store-key-bad").await;
    let oversized = "k".repeat(4097);

    for key in ["   ", oversized.as_str()] {
        let err = store_personal_key(
            &tokens(),
            &repo,
            &catalog,
            PersonalKeyRequest {
                user_id: &user,
                credential: "go",
                key,
                label: None,
            },
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, PersonalKeyError::Rejected(ref l) if l == "Go"),
            "{err:?}"
        );
    }
    let err = store_personal_key(
        &tokens(),
        &repo,
        &catalog,
        PersonalKeyRequest {
            user_id: &user,
            credential: "missing",
            key: "sk",
            label: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::NotPersonalCredential(_)),
        "{err:?}"
    );
    assert!(repo.list_metadata(&user).await.expect("list").is_empty());
    server.verify().await;
}
