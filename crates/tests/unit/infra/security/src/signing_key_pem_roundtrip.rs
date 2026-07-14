//! The cloud path syncs the signing key as a base64 PEM secret and the
//! bootstrap materializes it back into the authority. This verifies that
//! transform is lossless: the reloaded key keeps the same `kid`, so the JWKS a
//! restarted/secret-sourced machine publishes matches the tokens it mints.

use std::path::PathBuf;

use base64::Engine;
use systemprompt_security::keys::{KeyError, RsaSigningKey};
use uuid::Uuid;

#[test]
fn pem_base64_roundtrip_preserves_kid() {
    let key = RsaSigningKey::generate().expect("generate key");
    let pem = key.to_pkcs8_pem().expect("encode pem");

    let encoded = base64::engine::general_purpose::STANDARD.encode(pem.as_bytes());
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .expect("decode base64");
    let pem_back = String::from_utf8(decoded).expect("utf8 pem");

    let reloaded = RsaSigningKey::from_pkcs8_pem(&pem_back).expect("reload from pem");

    assert_eq!(key.kid(), reloaded.kid());
}

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sp-signkey-{tag}-{}.pem", Uuid::new_v4().simple()))
}

#[test]
fn debug_shows_kid_but_not_private_material() {
    let key = RsaSigningKey::generate().expect("generate key");
    let rendered = format!("{key:?}");
    assert!(rendered.contains("RsaSigningKey"));
    assert!(
        rendered.contains(key.kid()),
        "debug output surfaces the kid"
    );
    assert!(
        !rendered.contains("PRIVATE KEY"),
        "debug output must not leak PEM private key material"
    );
}

#[test]
fn write_pem_file_then_load_roundtrips_via_disk() {
    let key = RsaSigningKey::generate().expect("generate key");
    let path = temp_path("roundtrip");
    key.write_pem_file(&path).expect("write pem to disk");

    let reloaded = RsaSigningKey::load_from_pem_file(&path).expect("load pem from disk");
    assert_eq!(key.kid(), reloaded.kid());

    std::fs::remove_file(&path).ok();
}

#[test]
fn load_from_missing_file_is_an_io_error() {
    let path = temp_path("missing");
    let err = RsaSigningKey::load_from_pem_file(&path).expect_err("missing file must fail");
    assert!(
        matches!(err, KeyError::Io { .. }),
        "a missing PEM file must surface as an Io error, got {err:?}"
    );
}

#[test]
fn write_to_a_nonexistent_directory_is_an_io_error() {
    let key = RsaSigningKey::generate().expect("generate key");
    let path = PathBuf::from("/nonexistent-sp-dir-xyz").join("key.pem");
    let err = key
        .write_pem_file(&path)
        .expect_err("write into a missing dir must fail");
    assert!(
        matches!(err, KeyError::Io { .. }),
        "writing under a missing directory must surface as an Io error, got {err:?}"
    );
}
