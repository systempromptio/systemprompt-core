use std::collections::HashMap;

use systemprompt_config::{LoadedProfile, SecretsService};
use systemprompt_models::subprocess::SecretGrant;

use crate::fixture;

fn with_custom(base: &str) -> String {
    base.replacen(
        '{',
        "{\n  \"gcs_sa_key\": \"{}\",\n  \"stripe_key\": \"sk\",",
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
