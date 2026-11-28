use clap::ValueEnum;
use systemprompt_device::client::ClientError;
use systemprompt_device::protocol::Pat;
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::client::{ChangeKind, FeedChanges};
use systemprompt_models::documents::content_hash;
use systemprompt_scripted_instance::{ManifestFault, ScriptedInstance, TestPat};

use crate::feed::Feed;
use crate::manifest::{faulty, fetch};
use crate::support::client;

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
