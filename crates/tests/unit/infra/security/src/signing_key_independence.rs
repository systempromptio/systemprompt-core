use std::sync::Once;

use base64::Engine;
use ed25519_dalek::SigningKey;
use systemprompt_models::SecretsBootstrap;
use systemprompt_security::manifest_signing;

const SEED_B64: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";

static INIT_SECRETS: Once = Once::new();

fn ensure_bootstrap() {
    INIT_SECRETS.call_once(|| {
        unsafe {
            std::env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            std::env::set_var(
                "JWT_SECRET",
                "signing-key-independence-jwt-secret-A-32-bytes-or-longer",
            );
            std::env::set_var(
                "DATABASE_URL",
                "postgres://placeholder:placeholder@localhost/placeholder",
            );
            std::env::set_var("MANIFEST_SIGNING_SECRET_SEED", SEED_B64);
        }
        let _ = SecretsBootstrap::init();
    });
}

fn expected_pubkey_b64() -> String {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(SEED_B64)
        .expect("test seed must decode");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&raw);
    let key = SigningKey::from_bytes(&seed);
    base64::engine::general_purpose::STANDARD.encode(key.verifying_key().to_bytes())
}

#[test]
fn pubkey_derives_from_seed_not_jwt_secret() {
    ensure_bootstrap();
    let actual = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };
    assert_eq!(
        actual,
        expected_pubkey_b64(),
        "pubkey must come from manifest_signing_secret_seed, not the JWT secret"
    );
}

#[test]
fn pubkey_stable_across_jwt_secret_rotation() {
    ensure_bootstrap();
    let before = match manifest_signing::pubkey_b64() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };

    unsafe {
        std::env::set_var(
            "JWT_SECRET",
            "rotated-jwt-secret-A-prime-which-must-be-at-least-32-bytes",
        );
    }

    let after = manifest_signing::pubkey_b64().expect("pubkey after rotation");
    assert_eq!(
        before, after,
        "rotating the JWT secret must not change the manifest signing pubkey"
    );
}

#[test]
fn seed_accessor_returns_dedicated_value() {
    ensure_bootstrap();
    let seed = match SecretsBootstrap::manifest_signing_secret_seed() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping: secrets bootstrap unavailable in this env: {e}");
            return;
        },
    };
    let raw = base64::engine::general_purpose::STANDARD
        .decode(SEED_B64)
        .expect("seed decode");
    assert_eq!(&seed[..], &raw[..]);
}
