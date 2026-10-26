//! One personal key serves every endpoint of the one provider naming its
//! credential.
//!
//! Pins that storing the key probes every endpoint the credential serves
//! (all must accept; any rejection rejects the key), that a disabled
//! provider's endpoints are neither probed nor offered, and that only
//! personal credentials take a key.

use systemprompt_ai::{
    PersonalKeyError, credential_endpoints, personal_credential, personal_credentials,
    verify_personal_key,
};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig, ResolvedCatalog};
use systemprompt_models::providers::ClientFamily;
use systemprompt_test_fixtures::catalog::{CatalogFixture, model, personal, platform, priced};
use systemprompt_wire::WireProtocol;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn go_catalog(base: &str, go_enabled: bool) -> ResolvedCatalog {
    let go = ProviderConfig {
        enabled: go_enabled,
        ..ProviderConfig::new("go")
    };
    CatalogFixture::new()
        .credential("go", personal("Go"))
        .credential("platform", platform("platform"))
        .provider("go", go)
        .endpoint(
            "go",
            "chat",
            EndpointConfig::new(WireProtocol::OpenAiChat, format!("{base}/chat/v1")),
        )
        .endpoint(
            "go",
            "messages",
            EndpointConfig::new(WireProtocol::Anthropic, format!("{base}/messages/v1")),
        )
        .endpoint(
            "go",
            "mirror",
            EndpointConfig::new(WireProtocol::OpenAiChat, format!("{base}/mirror/v1")),
        )
        .provider("vendor", ProviderConfig::new("platform"))
        .endpoint(
            "vendor",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, format!("{base}/vendor/v1")),
        )
        .model(
            "go-chat",
            model(&[ClientFamily::OpenAi], 128_000, priced(1.0, 2.0, 0.1)),
        )
        .model(
            "go-messages",
            model(&[ClientFamily::Claude], 128_000, priced(1.0, 2.0, 0.1)),
        )
        .model("vendor-model", model(&[], 128_000, priced(1.0, 2.0, 0.1)))
        .deploy("go-chat", ["go/chat", "go/mirror"])
        .deploy("go-messages", ["go/messages"])
        .deploy("vendor-model", ["vendor/api"])
        .build()
}

async fn mount_models(server: &MockServer, prefix: &str, status: u16, calls: u64) {
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/models")))
        .respond_with(ResponseTemplate::new(status))
        .expect(calls)
        .mount(server)
        .await;
}

fn endpoint_names(catalog: &ResolvedCatalog, credential: &str) -> Vec<String> {
    let credential = catalog.credential(credential).expect("credential");
    credential_endpoints(catalog, credential)
        .iter()
        .map(|e| e.endpoint_ref().to_string())
        .collect()
}

#[test]
fn a_credentials_endpoints_are_every_endpoint_of_its_provider() {
    let catalog = go_catalog("https://go.example.test", true);
    assert_eq!(
        endpoint_names(&catalog, "go"),
        vec!["go/chat", "go/messages", "go/mirror"]
    );
}

#[test]
fn a_disabled_provider_contributes_no_endpoint() {
    let catalog = go_catalog("https://go.example.test", false);
    assert!(endpoint_names(&catalog, "go").is_empty());
    assert!(matches!(
        personal_credential(&catalog, "go"),
        Err(PersonalKeyError::NotPersonalCredential(_))
    ));
}

#[test]
fn only_personal_credentials_take_a_key() {
    let catalog = go_catalog("https://go.example.test", true);
    let ids: Vec<&str> = personal_credentials(&catalog)
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, vec!["go"]);
    assert!(matches!(
        personal_credential(&catalog, "platform"),
        Err(PersonalKeyError::NotPersonalCredential(_))
    ));
}

#[tokio::test]
async fn storing_a_key_probes_every_endpoint_of_the_credential() {
    let server = MockServer::start().await;
    mount_models(&server, "/chat/v1", 200, 1).await;
    mount_models(&server, "/messages/v1", 200, 1).await;
    mount_models(&server, "/mirror/v1", 200, 1).await;
    mount_models(&server, "/vendor/v1", 200, 0).await;
    let catalog = go_catalog(&server.uri(), true);
    let credential = personal_credential(&catalog, "go").expect("personal");

    verify_personal_key(
        &tokens(),
        &catalog,
        credential,
        "sk-go",
        &UserId::new("member-1"),
    )
    .await
    .expect("every endpoint accepts the key");
    server.verify().await;
}

#[tokio::test]
async fn a_disabled_providers_endpoint_is_not_probed() {
    let server = MockServer::start().await;
    mount_models(&server, "/chat/v1", 401, 0).await;
    mount_models(&server, "/messages/v1", 401, 0).await;
    mount_models(&server, "/mirror/v1", 401, 0).await;
    let catalog = go_catalog(&server.uri(), false);
    let credential = catalog.credential("go").expect("declared");

    let err = verify_personal_key(
        &tokens(),
        &catalog,
        credential,
        "sk-go",
        &UserId::new("member-1"),
    )
    .await
    .expect_err("a credential with no enabled endpoint takes no key");
    assert!(
        matches!(err, PersonalKeyError::NotPersonalCredential(_)),
        "{err:?}"
    );
    server.verify().await;
}

#[tokio::test]
async fn a_key_any_endpoint_refuses_is_rejected() {
    let server = MockServer::start().await;
    mount_models(&server, "/chat/v1", 200, 1).await;
    mount_models(&server, "/messages/v1", 401, 1).await;
    mount_models(&server, "/mirror/v1", 200, 0).await;
    let catalog = go_catalog(&server.uri(), true);
    let credential = personal_credential(&catalog, "go").expect("personal");

    let err = verify_personal_key(
        &tokens(),
        &catalog,
        credential,
        "sk-go",
        &UserId::new("member-1"),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, PersonalKeyError::Rejected(ref label) if label == "Go"),
        "{err:?}"
    );
}

fn tokens() -> std::sync::Arc<dyn systemprompt_traits::ProviderTokenCache> {
    std::sync::Arc::new(systemprompt_security::credential::TokenCache::new())
}
