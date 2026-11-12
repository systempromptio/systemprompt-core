//! The previous signing keys' secrets-document field: decoding and the
//! rotation write.

use chrono::{TimeZone, Utc};
use systemprompt_config::{
    PreviousSigningKey, SIGNING_KEY_PREVIOUS_KEYS, SIGNING_KEY_STAGED_PEM, SecretsBootstrapError,
    SigningKeyState, decode_previous_signing_keys, persist_signing_key_state,
};
use systemprompt_manifest::secrets::Secrets;

const PEPPER: &str = "pppppppppppppppppppppppppppppppppppppppp";
const PUBLIC_PEM: &str = "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n";

fn secrets(extra: &str) -> Secrets {
    Secrets::parse(&format!(
        r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a"{extra}}}"#
    ))
    .expect("fixture parses")
}

fn entry(kid: &str, year: i32) -> PreviousSigningKey {
    PreviousSigningKey {
        kid: kid.to_owned(),
        public_pem: PUBLIC_PEM.to_owned(),
        expires_at: Utc.with_ymd_and_hms(year, 1, 2, 3, 4, 5).unwrap(),
    }
}

fn with_previous(list: &[PreviousSigningKey]) -> Secrets {
    let encoded = serde_json::to_string(&serde_json::to_string(list).expect("list")).expect("str");
    secrets(&format!(r#","{SIGNING_KEY_PREVIOUS_KEYS}":{encoded}"#))
}

#[test]
fn no_previous_field_decodes_to_an_empty_list() {
    assert!(
        decode_previous_signing_keys(&secrets(""))
            .expect("decode")
            .is_empty()
    );
}

#[test]
fn every_previous_entry_decodes_in_order() {
    let list = vec![entry("k1", 2030), entry("k2", 2031), entry("k3", 2032)];
    assert_eq!(
        decode_previous_signing_keys(&with_previous(&list)).expect("decode"),
        list
    );
}

#[test]
fn a_malformed_previous_list_is_rejected() {
    for raw in [
        r#""not json""#,
        r#""[{\"kid\":\"k\"}]""#,
        r#""[{\"kid\":\"k\",\"public_pem\":\"p\",\"expires_at\":\"tomorrow\"}]""#,
    ] {
        let doc = secrets(&format!(r#","{SIGNING_KEY_PREVIOUS_KEYS}":{raw}"#));
        let err = decode_previous_signing_keys(&doc).expect_err("malformed list accepted");
        assert!(
            matches!(err, SecretsBootstrapError::SigningKeyPreviousInvalid(_)),
            "{err}"
        );
    }
}

#[test]
fn the_rotation_write_replaces_the_active_key_and_the_list_and_keeps_other_fields() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(
        &path,
        format!(r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a","custom_x":"kept"}}"#),
    )
    .expect("write");
    let rotation = SigningKeyState {
        active_private_pem: "new-private".to_owned(),
        staged_private_pem: None,
        previous: vec![entry("k1", 2030), entry("k2", 2031)],
    };
    persist_signing_key_state(&path, &rotation).expect("persist");

    let doc = Secrets::parse(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert!(doc.signing_key_pem.is_some());
    assert_eq!(doc.custom.get("custom_x").map(String::as_str), Some("kept"));
    assert_eq!(
        decode_previous_signing_keys(&doc).expect("decode"),
        rotation.previous
    );
}

#[test]
fn a_rotation_with_no_previous_keys_removes_the_field() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    let start = with_previous(&[entry("k1", 2030)]);
    std::fs::write(&path, serde_json::to_string(&start).expect("encode")).expect("write");
    let rotation = SigningKeyState {
        active_private_pem: "new-private".to_owned(),
        staged_private_pem: None,
        previous: Vec::new(),
    };
    persist_signing_key_state(&path, &rotation).expect("persist");
    let raw = std::fs::read_to_string(&path).expect("read");
    assert!(!raw.contains(SIGNING_KEY_PREVIOUS_KEYS), "{raw}");
}

#[test]
fn a_staged_key_is_written_and_cleared_by_the_state_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(
        &path,
        format!(r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a"}}"#),
    )
    .expect("write");
    let mut state = SigningKeyState {
        active_private_pem: "active".to_owned(),
        staged_private_pem: Some("staged".to_owned()),
        previous: Vec::new(),
    };
    persist_signing_key_state(&path, &state).expect("persist");
    let doc = Secrets::parse(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert!(doc.signing_key_staged_pem.is_some());

    state.staged_private_pem = None;
    persist_signing_key_state(&path, &state).expect("persist");
    let raw = std::fs::read_to_string(&path).expect("read");
    assert!(!raw.contains(SIGNING_KEY_STAGED_PEM), "{raw}");
}
