use std::sync::Arc;

use systemprompt_config::{LoadedProfile, SecretsService};
use systemprompt_manifest::Secrets;
use systemprompt_traits::SecretStore;

use crate::fixture;

fn with_custom(base: &str) -> String {
    base.replacen('{', "{\n  \"webhook_token\": \"tok-123\",", 1)
}

async fn loaded() -> (fixture::Fixture, Arc<SecretsService>) {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&with_custom(&fixture::secrets_json(Some(fixture::SEED)))),
    );
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile).await.expect("secrets");
    (fx, service)
}

fn store(service: &Arc<SecretsService>) -> &dyn SecretStore {
    service.as_ref()
}

#[tokio::test]
async fn custom_secrets_resolve_in_either_case() {
    let (_fx, service) = loaded().await;
    let store = store(&service);
    assert_eq!(
        store.secret("webhook_token").expect("present").expose(),
        "tok-123"
    );
    assert_eq!(
        store.secret("WEBHOOK_TOKEN").expect("present").expose(),
        "tok-123"
    );
    assert!(store.secret("absent_token").is_none());
}

#[tokio::test]
async fn key_names_list_populated_fields() {
    let (_fx, service) = loaded().await;
    let names = store(&service).key_names().expect("names");
    for key in [
        "webhook_token",
        "database_url",
        "oauth_at_rest_pepper",
        "manifest_signing_secret_seed",
        "encryption_master_key",
    ] {
        assert!(names.iter().any(|n| n == key), "{key} in {names:?}");
    }
    assert!(!names.iter().any(|n| n == "anthropic"));
}

#[tokio::test]
async fn seeds_decode_through_the_trait() {
    let (_fx, service) = loaded().await;
    let store = store(&service);
    assert_eq!(
        store.manifest_signing_secret_seed().expect("seed"),
        [0u8; 32]
    );
    assert_eq!(store.encryption_master_key().expect("key"), [0u8; 32]);
}

#[tokio::test]
async fn status_and_interval_reflect_the_file_source() {
    let (_fx, service) = loaded().await;
    let store = store(&service);
    let status = store.status();
    assert_eq!(status.generation, service.generation());
    assert!(status.last_error.is_none());
    assert!(store.refresh_interval().is_none());
}

#[tokio::test]
async fn unchanged_reload_does_not_swap() {
    let (_fx, service) = loaded().await;
    let before = service.generation();
    let outcome = store(&service).reload().await.expect("reload");
    assert!(!outcome.swapped);
    assert!(outcome.changed_keys.is_empty());
    assert_eq!(outcome.generation, before);
}

#[tokio::test]
async fn failed_reload_is_wrapped_and_recorded() {
    let (fx, service) = loaded().await;
    std::fs::write(&fx.secrets_path, "{ not json").expect("corrupt");
    let err = store(&service).reload().await.expect_err("corrupt file");
    assert!(err.to_string().contains("reloading the secrets"));
    assert!(std::error::Error::source(&err).is_some());
    assert!(store(&service).status().last_error.is_some());
}

#[tokio::test]
async fn missing_seed_and_key_are_refused_by_the_trait() {
    let (fx, _service) = loaded().await;
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let secrets: Secrets =
        serde_json::from_str(&fixture::secrets_json_with_master_key(None, None)).expect("secrets");
    let service = SecretsService::from_secrets(profile, secrets);
    let store = store(&service);
    let seed = store.manifest_signing_secret_seed().expect_err("no seed");
    assert!(seed.to_string().contains("manifest signing seed"));
    let key = store.encryption_master_key().expect_err("no key");
    assert!(key.to_string().contains("encryption master key"));
}
