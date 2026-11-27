use chrono::{DateTime, Utc};
use systemprompt_device::protocol::ids::AccountId;
use systemprompt_device::view::{
    AccountStatus, KeyFingerprint, SignInFailure, SyncErrorKind, SyncStatus,
};

#[test]
fn sync_status_serializes_with_a_status_tag() {
    let at = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let status = SyncStatus::Error {
        account: AccountId::new("a1"),
        kind: SyncErrorKind::KeyChanged,
        at,
    };
    let json = serde_json::to_value(&status).unwrap();
    assert_eq!(json["status"], "error");
    assert_eq!(json["kind"], "key-changed");
    assert_eq!(json["account"], "a1");
    assert_eq!(serde_json::from_value::<SyncStatus>(json).unwrap(), status);
    assert_eq!(
        serde_json::to_value(SyncStatus::Idle).unwrap(),
        serde_json::json!({ "status": "idle" })
    );
    assert_eq!(status.account().map(AccountId::as_str), Some("a1"));
}

#[test]
fn kinds_are_kebab_case() {
    assert_eq!(
        serde_json::to_value(SyncErrorKind::RateLimited).unwrap(),
        "rate-limited"
    );
    assert_eq!(
        serde_json::to_value(SignInFailure::RateLimited).unwrap(),
        "rate-limited"
    );
    assert_eq!(
        serde_json::to_value(AccountStatus::SignedIn).unwrap(),
        "signed-in"
    );
}

#[test]
fn fingerprints_come_from_key_bytes() {
    let fingerprint = KeyFingerprint::of_key(&[7u8; 32]);
    assert!(fingerprint.as_str().starts_with("sha256:"));
    assert_eq!(fingerprint.as_str().len(), 7 + 64);
}
