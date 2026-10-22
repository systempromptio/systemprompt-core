//! One personal key serves every entry that names it in `credential_from`.
//!
//! Pins that storing the key probes every sibling endpoint, not only the
//! owner's, and that the sibling entries share one token-cache key while two
//! different credentials never do.

use systemprompt_ai::{PersonalKeyError, UpstreamTarget, credential_siblings, verify_personal_key};
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

#[test]
fn entries_sharing_a_credential_share_one_token_cache_key() {
    let registry = go_registry("https://go.example.test");
    let user = UserId::new("member-1");
    let key = |name: &str| {
        let e = registry.find_provider(name).expect("entry");
        UpstreamTarget::resolve_personal(e, "sk-x", &user)
            .expect("resolves")
            .cache_key()
            .to_owned()
    };
    assert_eq!(key("go"), key("go-anthropic"));
    assert_ne!(key("go"), key("other"));

    let e = registry.find_provider("go").expect("entry");
    let other_user =
        UpstreamTarget::resolve_personal(e, "sk-x", &UserId::new("member-2")).expect("resolves");
    assert_ne!(other_user.cache_key(), key("go"));
}
