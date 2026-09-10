use systemprompt_security::manifest_signing::{
    canonical_manifest_bytes, key_id_for_pubkey, pubkey_b64_from_seed, sign_with_seed,
    verify_with_pubkey,
};

const SEED: [u8; 32] = [9u8; 32];
const OTHER_SEED: [u8; 32] = [11u8; 32];

#[test]
fn a_signature_made_with_a_seed_verifies_against_its_public_key() {
    let payload = b"services bundle manifest";
    let sig = sign_with_seed(&SEED, payload);
    let pubkey = pubkey_b64_from_seed(&SEED);

    verify_with_pubkey(&pubkey, payload, &sig).expect("signature verifies");
}

#[test]
fn a_tampered_payload_fails_verification() {
    let sig = sign_with_seed(&SEED, b"services bundle manifest");
    let pubkey = pubkey_b64_from_seed(&SEED);

    assert!(verify_with_pubkey(&pubkey, b"services bundle manifesT", &sig).is_err());
}

#[test]
fn a_different_key_fails_verification() {
    let payload = b"services bundle manifest";
    let sig = sign_with_seed(&SEED, payload);
    let other = pubkey_b64_from_seed(&OTHER_SEED);

    assert!(verify_with_pubkey(&other, payload, &sig).is_err());
}

#[test]
fn a_signature_from_another_payload_fails_verification() {
    let pubkey = pubkey_b64_from_seed(&SEED);
    let sig = sign_with_seed(&SEED, b"other manifest");

    assert!(verify_with_pubkey(&pubkey, b"services bundle manifest", &sig).is_err());
}

#[test]
fn a_public_key_that_is_not_base64_is_rejected() {
    let sig = sign_with_seed(&SEED, b"payload");
    let err = verify_with_pubkey("not base64!!", b"payload", &sig).expect_err("bad base64");
    assert!(err.to_string().contains("invalid base64"));
}

#[test]
fn a_signature_that_is_not_base64_is_rejected() {
    let pubkey = pubkey_b64_from_seed(&SEED);
    let err = verify_with_pubkey(&pubkey, b"payload", "not base64!!").expect_err("bad base64");
    assert!(err.to_string().contains("invalid base64"));
}

#[test]
fn a_public_key_of_the_wrong_length_is_rejected() {
    use base64::Engine;
    let short = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
    let sig = sign_with_seed(&SEED, b"payload");

    let err = verify_with_pubkey(&short, b"payload", &sig).expect_err("wrong length");
    assert!(err.to_string().contains("expected 32"));
}

#[test]
fn a_signature_of_the_wrong_length_is_rejected() {
    use base64::Engine;
    let short = base64::engine::general_purpose::STANDARD.encode([1u8; 32]);
    let pubkey = pubkey_b64_from_seed(&SEED);

    let err = verify_with_pubkey(&pubkey, b"payload", &short).expect_err("wrong length");
    assert!(err.to_string().contains("expected 64"));
}

#[test]
fn key_id_is_stable_and_key_specific() {
    let pubkey = pubkey_b64_from_seed(&SEED);
    let id = key_id_for_pubkey(&pubkey);

    assert_eq!(id.len(), 16);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(id, key_id_for_pubkey(&pubkey));
    assert_ne!(id, key_id_for_pubkey(&pubkey_b64_from_seed(&OTHER_SEED)));
}

#[test]
fn canonical_bytes_ignore_field_order() {
    #[derive(serde::Serialize)]
    struct A {
        b: u32,
        a: u32,
    }
    #[derive(serde::Serialize)]
    struct B {
        a: u32,
        b: u32,
    }

    let first = canonical_manifest_bytes(&A { b: 2, a: 1 }).expect("canonicalize");
    let second = canonical_manifest_bytes(&B { a: 1, b: 2 }).expect("canonicalize");
    assert_eq!(first, second);
}

#[test]
fn canonical_bytes_sign_and_verify_end_to_end() {
    #[derive(serde::Serialize)]
    struct Manifest {
        version: String,
        content_hash: String,
    }

    let manifest = Manifest {
        version: "1.0.0".to_owned(),
        content_hash: "abc".to_owned(),
    };
    let bytes = canonical_manifest_bytes(&manifest).expect("canonicalize");
    let sig = sign_with_seed(&SEED, &bytes);

    verify_with_pubkey(&pubkey_b64_from_seed(&SEED), &bytes, &sig).expect("verifies");
}
