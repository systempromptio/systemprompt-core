use std::sync::Once;

use base64::Engine;
use ed25519_dalek::SigningKey;
use systemprompt_config::SecretsBootstrap;

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
            if std::env::var_os("DATABASE_URL").is_none() {
                std::env::set_var(
                    "DATABASE_URL",
                    "postgres://placeholder:placeholder@localhost/placeholder",
                );
            }
            std::env::set_var("MANIFEST_SIGNING_SECRET_SEED", SEED_B64);
        }
        systemprompt_test_fixtures::ensure_test_bootstrap();
    });
}

fn manifest_keys() -> systemprompt_security::keys::SigningKeys {
    systemprompt_security::keys::SigningKeys::new(
        systemprompt_test_fixtures::test_key(systemprompt_test_fixtures::AUTHORITY_KEY_INDEX),
        &seed_bytes(),
    )
    .expect("build signing keys from the seed")
}

fn seed_bytes() -> [u8; 32] {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(SEED_B64)
        .expect("test seed must decode");
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&raw);
    seed
}

fn expected_pubkey_b64() -> String {
    let key = SigningKey::from_bytes(&seed_bytes());
    base64::engine::general_purpose::STANDARD.encode(key.verifying_key().to_bytes())
}

#[test]
fn pubkey_derives_from_seed_not_jwt_secret() {
    ensure_bootstrap();
    let actual = manifest_keys().manifest_pubkey_b64();
    assert_eq!(
        actual,
        expected_pubkey_b64(),
        "pubkey must come from manifest_signing_secret_seed, not the JWT secret"
    );
}

#[test]
fn pubkey_stable_across_jwt_secret_rotation() {
    ensure_bootstrap();
    let before = manifest_keys().manifest_pubkey_b64();

    unsafe {
        std::env::set_var(
            "JWT_SECRET",
            "rotated-jwt-secret-A-prime-which-must-be-at-least-32-bytes",
        );
    }

    let after = manifest_keys().manifest_pubkey_b64();
    assert_eq!(
        before, after,
        "rotating the JWT secret must not change the manifest signing pubkey"
    );
}

#[test]
fn seed_accessor_returns_dedicated_value() {
    ensure_bootstrap();
    let seed = SecretsBootstrap::manifest_signing_secret_seed()
        .expect("bootstrapped manifest signing seed");
    let raw = base64::engine::general_purpose::STANDARD
        .decode(SEED_B64)
        .expect("seed decode");
    assert_eq!(&seed[..], &raw[..]);
}
