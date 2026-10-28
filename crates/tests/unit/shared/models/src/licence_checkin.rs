use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use systemprompt_identifiers::{EnterpriseLicenseId, InstanceId, TenantId};
use systemprompt_models::licence::{
    CheckinSignatureError, LicenceCheckin, sign_checkin, verify_checkin,
};

const BODY: &[u8] = br#"{"instance_id":"node-a"}"#;
const SECRET: &[u8] = b"shared-secret";

#[test]
fn signature_is_versioned_hmac_sha256_hex_keyed_by_raw_secret() {
    assert_eq!(
        sign_checkin(b"what do ya want for nothing?", b"Jefe"),
        "v1=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

#[test]
fn signed_body_verifies() {
    let header = sign_checkin(BODY, SECRET);
    assert_eq!(verify_checkin(BODY, &header, SECRET), Ok(()));
}

#[test]
fn tampered_body_is_rejected() {
    let header = sign_checkin(BODY, SECRET);
    assert_eq!(
        verify_checkin(br#"{"instance_id":"node-b"}"#, &header, SECRET),
        Err(CheckinSignatureError::Mismatch)
    );
}

#[test]
fn wrong_secret_is_rejected() {
    let header = sign_checkin(BODY, SECRET);
    assert_eq!(
        verify_checkin(BODY, &header, b"other-secret"),
        Err(CheckinSignatureError::Mismatch)
    );
}

#[test]
fn missing_or_unknown_version_prefix_is_rejected() {
    let header = sign_checkin(BODY, SECRET);
    let digest = header.strip_prefix("v1=").unwrap();
    assert_eq!(
        verify_checkin(BODY, digest, SECRET),
        Err(CheckinSignatureError::UnknownVersion)
    );
    assert_eq!(
        verify_checkin(BODY, &format!("v2={digest}"), SECRET),
        Err(CheckinSignatureError::UnknownVersion)
    );
}

#[test]
fn non_hex_digest_is_rejected() {
    assert!(matches!(
        verify_checkin(BODY, "v1=zz", SECRET),
        Err(CheckinSignatureError::Hex(_))
    ));
}

#[test]
fn checkin_round_trips_through_json() {
    let checkin = LicenceCheckin {
        licence_id: Some(EnterpriseLicenseId::new("lic_123")),
        tenant_id: Some(TenantId::new("tenant_42")),
        instance_id: InstanceId::new("node-a"),
        core_version: "0.74.0".to_owned(),
        hosting_provider: "fly".to_owned(),
        external_host: Some("tenant.example.com".to_owned()),
        instances_by_role: BTreeMap::from([("api".to_owned(), 3), ("worker".to_owned(), 1)]),
        timestamp: Utc.with_ymd_and_hms(2026, 10, 28, 12, 0, 0).unwrap(),
    };
    let body = serde_json::to_vec(&checkin).unwrap();
    let back: LicenceCheckin = serde_json::from_slice(&body).unwrap();
    assert_eq!(back, checkin);
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["timestamp"], "2026-10-28T12:00:00Z");
}
