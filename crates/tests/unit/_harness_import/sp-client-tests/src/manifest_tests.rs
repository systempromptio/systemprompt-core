use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use sp_client::{ClientError, Incompatibility, ManifestFetch, ManifestKey, open_manifest, verify};
use sp_protocol::view::{KeyFingerprint, SyncErrorKind};
use sp_protocol::wire::manifest::SignedManifestEnvelope;
use sp_protocol::wire::paths;

use crate::support::{FakeInstance, FakeResponse, jwt};

const USER: &str = "1ecce4e3-bc87-441d-b374-ea29bab88da8";
const ETAG: &str = "\"3322c4ddc2a08588fa10168039bf874590bf50cb59d7b03575ac042cc70ead90\"";

fn payload(user: &str, schema: u32) -> String {
    format!(
        r#"{{"managed_mcp_servers":[],"min_schema_version":{schema},"plugins":[],"user_id":"{user}"}}"#
    )
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn manifest_key(seed: u8) -> ManifestKey {
    ManifestKey::from_raw(signing_key(seed).verifying_key().to_bytes())
}

fn envelope_json(signer: &SigningKey, payload_text: &str, served: &str) -> serde_json::Value {
    let signature = signer.sign(payload_text.as_bytes());
    json!({ "payload": served, "signature": STANDARD.encode(signature.to_bytes()) })
}

#[tokio::test]
async fn manifests_verify_and_revalidate() {
    let signer = signing_key(7);
    let payload_text = payload(USER, 1);
    let envelope = envelope_json(&signer, &payload_text, &payload_text);
    let pubkey = STANDARD.encode(signer.verifying_key().to_bytes());
    let fake = FakeInstance::start(move |request| match request.path() {
        paths::MANIFEST if request.header("if-none-match") == Some(ETAG) => {
            FakeResponse::status(304).header("ETag", ETAG)
        },
        paths::MANIFEST => FakeResponse::json(200, &envelope).header("ETag", ETAG),
        paths::PUBKEY => FakeResponse::json(200, &json!({ "pubkey": pubkey })),
        _ => FakeResponse::status(404),
    })
    .await;
    let client = fake.client();
    let key = client.pubkey().await.unwrap();
    assert_eq!(key, manifest_key(7));
    assert_eq!(
        key.fingerprint(),
        &KeyFingerprint::of_key(signer.verifying_key().as_bytes())
    );

    let fetched = client.manifest(&jwt(), &key, None, true).await.unwrap();
    let ManifestFetch::Fetched {
        envelope,
        manifest,
        etag,
    } = fetched
    else {
        panic!("expected a manifest");
    };
    assert_eq!(envelope.payload, payload_text);
    assert_eq!(manifest.user_id.as_str(), USER);
    assert_eq!(etag.as_deref(), Some(ETAG));
    let sent = fake.last();
    assert_eq!(
        sent.header("authorization"),
        Some("Bearer header.claims.signature")
    );
    assert_eq!(sent.header("cache-control"), Some("no-cache"));
    assert_eq!(
        open_manifest(&envelope, &key).unwrap().user_id.as_str(),
        USER
    );

    let again = client
        .manifest(&jwt(), &key, Some(ETAG), false)
        .await
        .unwrap();
    assert!(matches!(again, ManifestFetch::NotModified));
    assert!(fake.last().header("cache-control").is_none());
}

#[tokio::test]
async fn a_tampered_payload_is_refused() {
    let signer = signing_key(7);
    let tampered = envelope_json(&signer, &payload(USER, 1), &payload("someone-else", 1));
    let same_key = STANDARD.encode(signer.verifying_key().to_bytes());
    let fake = FakeInstance::start(move |request| match request.path() {
        paths::MANIFEST => FakeResponse::json(200, &tampered),
        paths::PUBKEY => FakeResponse::json(200, &json!({ "pubkey": same_key })),
        _ => FakeResponse::status(404),
    })
    .await;
    let error = fake
        .client()
        .manifest(&jwt(), &manifest_key(7), None, false)
        .await
        .unwrap_err();
    assert!(matches!(error, ClientError::SignatureInvalid { .. }));
    assert_eq!(error.sync_error_kind(), SyncErrorKind::Invalid);
}

#[tokio::test]
async fn a_new_signing_key_is_a_key_change() {
    let rotated = signing_key(9);
    let payload_text = payload(USER, 1);
    let envelope = envelope_json(&rotated, &payload_text, &payload_text);
    let served = STANDARD.encode(rotated.verifying_key().to_bytes());
    let fake = FakeInstance::start(move |request| match request.path() {
        paths::MANIFEST => FakeResponse::json(200, &envelope),
        paths::PUBKEY => FakeResponse::json(200, &json!({ "pubkey": served })),
        _ => FakeResponse::status(404),
    })
    .await;
    let pinned = manifest_key(7);
    let error = fake
        .client()
        .manifest(&jwt(), &pinned, None, false)
        .await
        .unwrap_err();
    let ClientError::KeyChanged {
        pinned: was,
        offered,
    } = &error
    else {
        panic!("expected a key change, got {error:?}");
    };
    assert_eq!(was, pinned.fingerprint());
    assert_eq!(offered, manifest_key(9).fingerprint());
    assert_eq!(error.sync_error_kind(), SyncErrorKind::KeyChanged);
}

#[test]
fn a_newer_schema_is_incompatible_after_verification() {
    let signer = signing_key(7);
    let payload_text = payload(USER, 2);
    let envelope: SignedManifestEnvelope =
        serde_json::from_value(envelope_json(&signer, &payload_text, &payload_text)).unwrap();
    let error = open_manifest(&envelope, &manifest_key(7)).unwrap_err();
    assert!(matches!(
        error,
        ClientError::Incompatible {
            what: Incompatibility::ManifestSchema { found: 2 },
            ..
        }
    ));
    let forged: SignedManifestEnvelope =
        serde_json::from_value(json!({ "payload": payload_text, "signature": "not base64!" }))
            .unwrap();
    assert!(matches!(
        open_manifest(&forged, &manifest_key(7)),
        Err(ClientError::SignatureInvalid { .. })
    ));
}

#[test]
fn manifest_keys_round_trip_and_hide_their_bytes() {
    let key = manifest_key(7);
    let encoded = key.to_base64();
    let decoded = ManifestKey::from_base64(&encoded).unwrap();
    assert_eq!(decoded, key);
    assert_eq!(decoded.raw(), &signing_key(7).verifying_key().to_bytes());
    assert!(ManifestKey::from_base64("not a key").is_err());
    let shown = format!("{key:?}");
    assert!(shown.contains(key.fingerprint().as_str()));
    assert!(!shown.contains(&encoded));
}

#[test]
fn verification_is_over_the_exact_signed_bytes() {
    let signer = signing_key(7);
    let payload_text = payload(USER, 1);
    let exact: SignedManifestEnvelope =
        serde_json::from_value(envelope_json(&signer, &payload_text, &payload_text)).unwrap();
    assert!(verify(&exact, &manifest_key(7)).is_ok());
    let respaced = payload_text.replace(',', ", ");
    let altered: SignedManifestEnvelope =
        serde_json::from_value(envelope_json(&signer, &payload_text, &respaced)).unwrap();
    assert!(matches!(
        verify(&altered, &manifest_key(7)),
        Err(ClientError::SignatureInvalid { .. })
    ));
    assert!(matches!(
        verify(&exact, &manifest_key(8)),
        Err(ClientError::SignatureInvalid { .. })
    ));
}
