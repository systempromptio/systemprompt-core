use chrono::Utc;
use systemprompt_device::client::{ClientError, ManifestFetch};
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::version::{dated_beyond_skew, is_newer};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_scripted_instance::{
    ManifestFault, ManifestFixture, ModelFixture, ScriptedInstance,
};

use crate::support::signed_in;

async fn faulty(fault: ManifestFault) -> ScriptedInstance {
    ScriptedInstance::builder()
        .manifest(ManifestFixture::minimal().with_fault(fault))
        .start()
        .await
        .unwrap()
}

async fn fetch(instance: &ScriptedInstance) -> Result<Box<Manifest>, ClientError> {
    let (client, session) = signed_in(instance).await;
    let key = client.pubkey().await?;
    match client.manifest(&session.jwt, &key, None, false).await? {
        ManifestFetch::Fetched { manifest, .. } => Ok(manifest),
        ManifestFetch::NotModified => panic!("a fetch without an ETag carries a manifest"),
    }
}

#[tokio::test]
async fn the_minimal_manifest_is_acceptable_as_a_first_manifest() {
    let instance = ScriptedInstance::builder()
        .models([ModelFixture::chat("m-b"), ModelFixture::messages("m-a")])
        .start()
        .await
        .unwrap();
    let manifest = fetch(&instance).await.unwrap();
    assert_eq!(manifest.user_id, instance.user().user_id);
    assert!(!dated_beyond_skew(manifest.not_before, Utc::now(), false));
    assert!(is_newer(&manifest.manifest_version, None));
    assert_eq!(manifest.enabled_hosts, vec![HostKind::Harness]);
    let families = manifest.model_families(HostKind::Harness).unwrap();
    assert_eq!(families, ["claude", "openai"]);
}

#[tokio::test]
async fn a_bad_signature_is_refused_by_the_verifier() {
    let refused = fetch(&faulty(ManifestFault::BadSignature).await).await;
    assert!(matches!(refused, Err(ClientError::SignatureInvalid { .. })));
}

#[tokio::test]
async fn a_changed_key_is_reported_as_a_key_change() {
    let refused = fetch(&faulty(ManifestFault::ChangedKey).await).await;
    assert!(matches!(refused, Err(ClientError::KeyChanged { .. })));
}

#[tokio::test]
async fn a_wrong_user_verifies_and_names_another_user() {
    let instance = faulty(ManifestFault::WrongUser).await;
    let manifest = fetch(&instance).await.unwrap();
    assert_ne!(manifest.user_id, instance.user().user_id);
}

#[tokio::test]
async fn a_future_not_before_is_beyond_the_skew_window() {
    let manifest = fetch(&faulty(ManifestFault::FutureNotBefore).await)
        .await
        .unwrap();
    assert!(dated_beyond_skew(manifest.not_before, Utc::now(), true));
}

#[tokio::test]
async fn a_stale_version_follows_the_first_fetch() {
    let instance = faulty(ManifestFault::StaleVersion).await;
    let first = fetch(&instance).await.unwrap();
    let second = fetch(&instance).await.unwrap();
    assert!(is_newer(&first.manifest_version, None));
    assert!(!is_newer(
        &second.manifest_version,
        Some(&first.manifest_version)
    ));
    assert!(!dated_beyond_skew(second.not_before, Utc::now(), true));
}
