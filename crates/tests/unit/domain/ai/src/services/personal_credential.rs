//! One personal key serves every entry that names it in `credential_from`.
//!
//! Pins that storing the key probes every sibling endpoint, not only the
//! owner's.

use systemprompt_ai::{PersonalKeyError, credential_siblings, verify_personal_key};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::{ProviderEntry, ProviderRegistry};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn entry(yaml: &str) -> ProviderEntry {
    serde_yaml::from_str(yaml).expect("provider entry")
}

fn go_registry(base: &str) -> ProviderRegistry {
    let chat = entry(&format!(
        "name: go\nwire: openai-chat\nsurface: openai\nendpoint: {base}/chat/v1\ncredential: \
         user\nmodels:\n  - id: go-chat\n"
    ));
    let messages = entry(&format!(
        "name: go-anthropic\nwire: anthropic\nsurface: openai\nendpoint: \
         {base}/messages/v1\ncredential: user\ncredential_from: go\nmodels:\n  - id: \
         go-messages\n"
    ));
    let other = entry(&format!(
        "name: other\nwire: anthropic\nsurface: openai\nendpoint: {base}/other/v1\ncredential: \
         user\nmodels:\n  - id: other-model\n"
    ));
    ProviderRegistry {
        providers: vec![chat, messages, other],
    }
}

fn owner(registry: &ProviderRegistry) -> &ProviderEntry {
    registry.find_provider("go").expect("owner entry")
}

async fn mount_models(server: &MockServer, prefix: &str, status: u16) {
    Mock::given(method("GET"))
        .and(path(format!("{prefix}/models")))
        .respond_with(ResponseTemplate::new(status))
        .expect(1)
        .mount(server)
        .await;
}

#[test]
fn siblings_are_the_owner_and_entries_naming_it() {
    let registry = go_registry("https://go.example.test");
    let names: Vec<&str> = credential_siblings(&registry, owner(&registry))
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, vec!["go", "go-anthropic"]);
}

#[tokio::test]
async fn storing_a_shared_key_probes_every_sibling_endpoint() {
    let server = MockServer::start().await;
    mount_models(&server, "/chat/v1", 200).await;
    mount_models(&server, "/messages/v1", 200).await;
    let registry = go_registry(&server.uri());

    verify_personal_key(
        &registry,
        owner(&registry),
        "sk-go",
        &UserId::new("member-1"),
    )
    .await
    .expect("both endpoints accept the key");
}

#[tokio::test]
async fn a_key_one_sibling_refuses_is_rejected() {
    let server = MockServer::start().await;
    mount_models(&server, "/chat/v1", 200).await;
    mount_models(&server, "/messages/v1", 401).await;
    let registry = go_registry(&server.uri());

    let err = verify_personal_key(
        &registry,
        owner(&registry),
        "sk-go",
        &UserId::new("member-1"),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, PersonalKeyError::Rejected(_)), "{err:?}");
}
