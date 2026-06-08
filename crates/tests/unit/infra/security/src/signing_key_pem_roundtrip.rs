//! The cloud path syncs the signing key as a base64 PEM secret and the
//! bootstrap materializes it back into the authority. This verifies that
//! transform is lossless: the reloaded key keeps the same `kid`, so the JWKS a
//! restarted/secret-sourced machine publishes matches the tokens it mints.

use base64::Engine;
use systemprompt_security::keys::RsaSigningKey;

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
