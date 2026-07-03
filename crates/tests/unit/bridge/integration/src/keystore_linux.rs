//! Tests for the Linux device-cert keystore: `SP_BRIDGE_DEVICE_CERT`-driven
//! load with PEM-to-DER conversion and raw-DER fallback.

use sha2::{Digest, Sha256};
use systemprompt_bridge::auth::keystore::{KeystoreError, platform_source, sha256_der};
use tempfile::tempdir;

const DER: &[u8] = &[0x30, 0x82, 0x01, 0x0a, 0xde, 0xad, 0xbe, 0xef];

fn hex_sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHA[(n >> 18) as usize & 63] as char);
        out.push(ALPHA[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHA[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHA[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[test]
fn unset_env_yields_not_configured() {
    temp_env::with_var("SP_BRIDGE_DEVICE_CERT", None::<&str>, || {
        let err = platform_source().load().unwrap_err();
        assert!(
            matches!(err, KeystoreError::NotConfigured(_)),
            "expected NotConfigured, got {err:?}"
        );
    });
}

#[test]
fn missing_file_yields_io_error() {
    temp_env::with_var(
        "SP_BRIDGE_DEVICE_CERT",
        Some("/nonexistent/cert.pem"),
        || {
            let err = platform_source().load().unwrap_err();
            assert!(
                matches!(err, KeystoreError::Io(_)),
                "expected Io, got {err:?}"
            );
        },
    );
}

#[test]
fn raw_der_file_fingerprints_bytes_directly() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("cert.der");
    std::fs::write(&path, DER).unwrap();

    temp_env::with_var("SP_BRIDGE_DEVICE_CERT", Some(path.as_os_str()), || {
        let cert = platform_source().load().unwrap();
        assert_eq!(cert.fingerprint.as_str(), hex_sha(DER));
    });
}

#[test]
fn pem_file_is_decoded_to_der_before_fingerprinting() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("cert.pem");
    let body = base64_encode(DER);
    let pem = format!("-----BEGIN CERTIFICATE-----\n{body}\n-----END CERTIFICATE-----\n");
    std::fs::write(&path, &pem).unwrap();

    temp_env::with_var("SP_BRIDGE_DEVICE_CERT", Some(path.as_os_str()), || {
        let cert = platform_source().load().unwrap();
        assert_eq!(
            cert.fingerprint.as_str(),
            hex_sha(DER),
            "PEM body must be base64-decoded to DER before hashing"
        );
    });
}

#[test]
fn pem_with_invalid_base64_falls_back_to_raw_bytes() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("cert.pem");
    let pem = "-----BEGIN CERTIFICATE-----\n!!!not-base64!!!\n-----END CERTIFICATE-----\n";
    std::fs::write(&path, pem).unwrap();

    temp_env::with_var("SP_BRIDGE_DEVICE_CERT", Some(path.as_os_str()), || {
        let cert = platform_source().load().unwrap();
        assert_eq!(cert.fingerprint.as_str(), hex_sha(pem.as_bytes()));
    });
}

#[test]
fn sha256_der_produces_lowercase_hex() {
    let fp = sha256_der(b"abc").unwrap();
    assert_eq!(
        fp.as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
