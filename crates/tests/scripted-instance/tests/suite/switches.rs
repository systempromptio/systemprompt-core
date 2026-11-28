use clap::ValueEnum;
use std::path::PathBuf;

use systemprompt_device::client::{ClientError, ManifestFetch};
use systemprompt_device::protocol::Pat;
use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_device::protocol::version::is_newer;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::HostExtensionId;
use systemprompt_models::documents::client::{ChangeKind, FeedChanges};
use systemprompt_models::documents::content_hash;
use systemprompt_scripted_instance::{ManifestFault, ManifestFixture, ScriptedInstance, TestPat};

use crate::feed::Feed;
use crate::manifest::{faulty, fetch};
use crate::support::{client, signed_in};

const FAULTS: [(&str, ManifestFault); 5] = [
    ("stale-version", ManifestFault::StaleVersion),
    ("future-not-before", ManifestFault::FutureNotBefore),
    ("wrong-user", ManifestFault::WrongUser),
    ("changed-key", ManifestFault::ChangedKey),
    ("bad-signature", ManifestFault::BadSignature),
];

#[tokio::test]
async fn a_fault_is_taken_by_name_and_served() {
    assert_eq!(ManifestFault::value_variants().len(), FAULTS.len());
    for (name, fault) in FAULTS {
        assert_eq!(ManifestFault::from_str(name, false), Ok(fault));
    }
    let unknown = ManifestFault::from_str("expired", false);
    assert!(unknown.is_err());

    let named = ManifestFault::from_str("bad-signature", false).unwrap();
    let refused = fetch(&faulty(named).await).await;
    assert!(matches!(refused, Err(ClientError::SignatureInvalid { .. })));
}

#[tokio::test]
async fn the_token_the_builder_is_given_is_the_only_one_accepted() {
    let rotated = TestPat::new("sp-live-rotated.instance-pat");
    let instance = ScriptedInstance::builder()
        .pat(rotated.clone())
        .start()
        .await
        .unwrap();
    assert_eq!(instance.pat(), &rotated);
    let client = client(&instance);

    let accepted = Pat::new(rotated.as_str()).unwrap();
    client.mint_session(&accepted, None).await.unwrap();

    let stored = Pat::new(TestPat::fixture().as_str()).unwrap();
    let refused = client.mint_session(&stored, None).await.unwrap_err();
    assert!(refused.is_unauthorized());
}

#[tokio::test]
async fn the_drive_starts_with_the_documents_the_builder_is_given() {
    let instance = ScriptedInstance::builder()
        .document("notes/a.md", b"one".to_vec())
        .document("b.txt", b"draft".to_vec())
        .document("b.txt", b"two".to_vec())
        .start()
        .await
        .unwrap();
    let feed = Feed::over(instance).await;

    let listed = feed.manifest().await;
    assert_eq!(listed.cursor, "3");
    let mut held: Vec<(&str, &str)> = listed
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.content_sha256.as_str()))
        .collect();
    held.sort_unstable();
    let (one, two) = (content_hash(b"one"), content_hash(b"two"));
    assert_eq!(
        held,
        [("b.txt", two.as_str()), ("notes/a.md", one.as_str())]
    );

    let page: FeedChanges = feed.changes("0").await.json().await.unwrap();
    let kinds: Vec<&ChangeKind> = page.changes.iter().map(|change| &change.kind).collect();
    assert_eq!(
        kinds,
        [
            &ChangeKind::Created,
            &ChangeKind::Created,
            &ChangeKind::Updated
        ]
    );

    let blob = feed
        .http
        .get(format!(
            "{}/{}",
            feed.url(ApiPaths::BRIDGE_DRIVE_BLOB),
            two.as_str()
        ))
        .bearer_auth(&feed.token)
        .send()
        .await
        .unwrap();
    assert_eq!(blob.bytes().await.unwrap().as_ref(), b"two");

    let unbased = feed.commit(None, "notes/a.md", None, b"three").await;
    assert_eq!(unbased.status().as_u16(), 409);
}

fn key_file(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("scripted-instance-keys")
        .join(name);
    if let Err(error) = std::fs::remove_file(&path) {
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
    }
    path
}

async fn fetched(
    instance: &ScriptedInstance,
    key: &systemprompt_device::client::ManifestKey,
) -> Result<Box<Manifest>, ClientError> {
    let (client, session) = signed_in(instance).await;
    match client.manifest(&session.jwt, key, None, false).await? {
        ManifestFetch::Fetched { manifest, .. } => Ok(manifest),
        ManifestFetch::NotModified => panic!("a fetch without an ETag carries a manifest"),
    }
}

#[tokio::test]
async fn a_restart_on_the_same_key_file_is_trusted_and_serves_an_older_version_at_once() {
    let path = key_file("restart.keys");
    let first = ScriptedInstance::builder()
        .keys(&path)
        .start()
        .await
        .unwrap();
    let pinned = client(&first).pubkey().await.unwrap();
    let healthy = fetched(&first, &pinned).await.unwrap();
    first.shutdown().await.unwrap();
    assert_eq!(std::fs::read(&path).unwrap().len(), 96);

    let restarted = ScriptedInstance::builder()
        .keys(&path)
        .manifest(ManifestFixture::minimal().with_fault(ManifestFault::StaleVersion))
        .start()
        .await
        .unwrap();
    let stale = fetched(&restarted, &pinned).await.unwrap();
    assert!(stale.manifest_version < healthy.manifest_version);
    assert!(!is_newer(
        &stale.manifest_version,
        Some(&healthy.manifest_version)
    ));

    let other = ScriptedInstance::builder()
        .keys(key_file("other.keys"))
        .start()
        .await
        .unwrap();
    let refused = fetched(&other, &pinned).await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn a_host_extension_is_listed_for_the_harness_host() {
    let drive = HostExtensionId::try_new("drive").unwrap();
    let instance = ScriptedInstance::builder()
        .manifest(
            ManifestFixture::minimal()
                .with_extension(drive.clone())
                .with_extension(drive.clone()),
        )
        .start()
        .await
        .unwrap();
    let key = client(&instance).pubkey().await.unwrap();
    let manifest = fetched(&instance, &key).await.unwrap();
    assert_eq!(
        manifest.extensions_for(HostKind::Harness),
        Some([drive].as_slice())
    );

    let plain = ScriptedInstance::builder().start().await.unwrap();
    let key = client(&plain).pubkey().await.unwrap();
    let manifest = fetched(&plain, &key).await.unwrap();
    assert_eq!(manifest.extensions_for(HostKind::Harness), None);
}
