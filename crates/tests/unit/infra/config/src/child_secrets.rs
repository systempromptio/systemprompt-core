use std::collections::HashMap;

use systemprompt_config::{GrantedSecrets, LoadedProfile, SecretsService};
use systemprompt_models::subprocess::SecretGrant;
use systemprompt_traits::SecretStore;

use crate::fixture;

fn with_custom(base: &str) -> String {
    base.replacen(
        '{',
        "{\n  \"gcs_sa_key\": \"{}\",\n  \"stripe_key\": \"sk\",\n  \"LICENCE_SIGNING_SECRET\": \"licence\",",
        1,
    )
}

async fn service(storage: &str) -> (fixture::Fixture, std::sync::Arc<SecretsService>) {
    let fx = fixture::write_tree(
        fixture::FILE_SECRETS,
        Some(&with_custom(&fixture::secrets_json(Some(fixture::SEED)))),
    );
    let profile = std::fs::read_to_string(&fx.profile_path).expect("read profile");
    std::fs::write(&fx.profile_path, format!("{profile}{storage}")).expect("write profile");
    let profile = LoadedProfile::from_path(&fx.profile_path).expect("profile");
    let service = SecretsService::load(profile).await.expect("secrets");
    (fx, service)
}

const GCS_WITH_SECRET: &str =
    "storage:\n  backend: gcs\n  bucket: media-bucket\n  credentials:\n    secret: gcs_sa_key\n";

fn env(service: &SecretsService, explicit: &[SecretGrant]) -> HashMap<String, String> {
    service
        .child_env(explicit, &[])
        .expect("child env")
        .into_iter()
        .collect()
}

#[tokio::test]
async fn the_storage_grant_carries_the_secret_the_profile_storage_names() {
    let (_fx, service) = service(GCS_WITH_SECRET).await;
    let granted = env(&service, &[SecretGrant::Storage]);
    assert_eq!(granted.get("GCS_SA_KEY").map(String::as_str), Some("{}"));
    let ungranted = env(&service, &[]);
    assert!(!ungranted.contains_key("GCS_SA_KEY"));
    assert!(!ungranted.contains_key("STRIPE_KEY"));
}

#[tokio::test]
async fn the_storage_grant_under_workload_identity_adds_nothing() {
    let (_fx, service) = service("storage:\n  backend: gcs\n  bucket: media-bucket\n").await;
    let granted = env(&service, &[SecretGrant::Storage]);
    assert!(!granted.contains_key("GCS_SA_KEY"));
}

#[tokio::test]
async fn a_derived_model_credential_naming_an_api_held_secret_is_refused() {
    let (_fx, service) = service("").await;
    assert!(
        service
            .child_env(&[], &["oauth_at_rest_pepper".to_owned()])
            .is_err()
    );
}

const API_HELD_ENV: [&str; 7] = [
    "OAUTH_AT_REST_PEPPER",
    "MANIFEST_SIGNING_SECRET_SEED",
    "SIGNING_KEY_PEM",
    "SIGNING_KEY_STAGED_PEM",
    "SIGNING_KEY_PREVIOUS_KEYS",
    "LICENCE_SIGNING_SECRET",
    "ENCRYPTION_MASTER_KEY",
];

#[tokio::test]
async fn a_host_child_env_carries_no_api_held_secret_and_no_ungranted_master_key() {
    let (_fx, service) = service("").await;
    let granted = env(&service, &[]);
    for name in API_HELD_ENV {
        assert!(!granted.contains_key(name), "{name} reached a host child");
    }
    assert!(granted.contains_key("DATABASE_URL"));
    assert!(
        granted
            .values()
            .all(|value| value != fixture::PEPPER && value != fixture::MASTER_KEY),
        "an API-held value rode under another name"
    );
}

#[tokio::test]
async fn the_master_key_reaches_a_host_child_only_when_granted() {
    let (_fx, service) = service("").await;
    let granted = env(&service, &[SecretGrant::EncryptionMasterKey]);
    assert_eq!(
        granted.get("ENCRYPTION_MASTER_KEY").map(String::as_str),
        Some(fixture::MASTER_KEY)
    );
    for name in API_HELD_ENV
        .iter()
        .filter(|name| **name != "ENCRYPTION_MASTER_KEY")
    {
        assert!(!granted.contains_key(*name), "{name} reached a host child");
    }
    let store = GrantedSecrets::from_lookup(|name| granted.get(name).cloned()).expect("granted");
    assert!(store.encryption_master_key().is_ok());
}

#[tokio::test]
async fn an_agent_child_env_with_model_credentials_carries_no_api_held_secret() {
    let (_fx, service) = service("").await;
    let agent: HashMap<String, String> = service
        .child_env(
            &[SecretGrant::from_secret_name("stripe_key").expect("grant")],
            &["anthropic_api_key".to_owned()],
        )
        .expect("agent env")
        .into_iter()
        .collect();
    for name in API_HELD_ENV {
        assert!(!agent.contains_key(name), "{name} reached an agent child");
    }
    assert!(
        agent
            .values()
            .all(|value| value != fixture::PEPPER && value != fixture::MASTER_KEY),
        "an API-held value rode under another name"
    );
}

#[tokio::test]
async fn granted_secrets_hold_only_the_grants_and_refuse_the_seed_and_an_ungranted_master_key() {
    let (_fx, service) = service("").await;
    let granted = env(&service, &[SecretGrant::Custom("stripe_key".to_owned())]);
    let store = GrantedSecrets::from_lookup(|name| granted.get(name).cloned()).expect("granted");
    assert_eq!(store.granted_names(), vec!["stripe_key".to_owned()]);
    assert_eq!(
        store.secret("stripe_key").map(|s| s.expose().to_owned()),
        Some("sk".to_owned())
    );
    assert!(store.secret("gcs_sa_key").is_none());
    assert!(store.manifest_signing_secret_seed().is_err());
    assert!(store.encryption_master_key().is_err());
    assert_eq!(store.credentials().database_url, fixture::DB_URL);
}

#[test]
fn granted_secrets_need_a_database_url() {
    assert!(GrantedSecrets::from_lookup(|_| None).is_err());
}
