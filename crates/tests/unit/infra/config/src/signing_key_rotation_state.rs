//! The retired signing key's secrets-document fields: decoding and the
//! rotation write.

use base64::Engine;
use chrono::{TimeZone, Utc};
use systemprompt_config::{
    PreviousSigningKey, SecretsBootstrapError, SigningKeyRotation, decode_previous_signing_key,
    persist_signing_key_rotation,
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

fn b64(value: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(value)
}

#[test]
fn no_previous_fields_decode_to_no_retired_key() {
    assert_eq!(
        decode_previous_signing_key(&secrets("")).expect("decode"),
        None
    );
}

#[test]
fn both_previous_fields_decode_to_the_retired_key() {
    let doc = secrets(&format!(
        r#","signing_key_previous_public_pem":"{}","signing_key_previous_expires_at":"2030-01-02T03:04:05Z""#,
        b64(PUBLIC_PEM)
    ));
    let previous = decode_previous_signing_key(&doc)
        .expect("decode")
        .expect("present");
    assert_eq!(previous.public_pem, PUBLIC_PEM);
    assert_eq!(
        previous.expires_at,
        Utc.with_ymd_and_hms(2030, 1, 2, 3, 4, 5).unwrap()
    );
}

#[test]
fn a_previous_key_without_an_expiry_is_rejected() {
    let doc = secrets(&format!(
        r#","signing_key_previous_public_pem":"{}""#,
        b64(PUBLIC_PEM)
    ));
    let err = decode_previous_signing_key(&doc).expect_err("unbounded retired key accepted");
    assert!(
        matches!(err, SecretsBootstrapError::SigningKeyPreviousIncomplete),
        "{err}"
    );
}

#[test]
fn an_expiry_without_a_previous_key_is_rejected() {
    let doc = secrets(r#","signing_key_previous_expires_at":"2030-01-02T03:04:05Z""#);
    let err = decode_previous_signing_key(&doc).expect_err("orphan expiry accepted");
    assert!(
        matches!(err, SecretsBootstrapError::SigningKeyPreviousIncomplete),
        "{err}"
    );
}

#[test]
fn a_malformed_expiry_is_rejected() {
    let doc = secrets(&format!(
        r#","signing_key_previous_public_pem":"{}","signing_key_previous_expires_at":"tomorrow""#,
        b64(PUBLIC_PEM)
    ));
    let err = decode_previous_signing_key(&doc).expect_err("malformed expiry accepted");
    assert!(
        matches!(
            err,
            SecretsBootstrapError::SigningKeyPreviousExpiryInvalid(_)
        ),
        "{err}"
    );
}

#[test]
fn the_rotation_write_replaces_the_active_key_and_keeps_other_fields() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"oauth_at_rest_pepper":"{PEPPER}","database_url":"postgres://a","signing_key_pem":"{}","custom_x":"kept"}}"#,
            b64("old-private")
        ),
    )
    .expect("write");
    let expires_at = Utc.with_ymd_and_hms(2031, 6, 7, 8, 9, 10).unwrap();
    let rotation = SigningKeyRotation {
        active_private_pem: "new-private".to_owned(),
        previous: PreviousSigningKey {
            public_pem: PUBLIC_PEM.to_owned(),
            expires_at,
        },
    };
    persist_signing_key_rotation(&path, &rotation).expect("persist");

    let doc = Secrets::parse(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(
        doc.signing_key_pem.as_deref(),
        Some(b64("new-private").as_str())
    );
    assert_eq!(doc.custom.get("custom_x").map(String::as_str), Some("kept"));
    let previous = decode_previous_signing_key(&doc)
        .expect("decode")
        .expect("present");
    assert_eq!(previous, rotation.previous);
}
