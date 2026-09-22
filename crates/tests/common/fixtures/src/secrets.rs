//! Shared secrets-bootstrap helper for integration tests.
//!
//! Every test that touches the JWT/OAuth/security path must invoke
//! [`ensure_test_secrets_bootstrap`] before its first DB call.  The helper is
//! idempotent under [`std::sync::Once`] and uses the subprocess bootstrap path
//! so that `SecretsBootstrap::try_init` matches how production deployments load
//! deployment secrets in air-gapped / container modes.

use std::env;
use std::sync::Once;

use systemprompt_config::SecretsBootstrap;

const TEST_OAUTH_AT_REST_PEPPER: &str = "test_oauth_at_rest_pepper_for_integration_tests_zzz";
const TEST_MANIFEST_SIGNING_SEED: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
// The at-rest cipher key, as the full fixture bootstrap also installs it.
// Anything sealing a column (`systemprompt_security::at_rest`) needs it
// resolvable, so the secrets-only bootstrap must install it too.
const TEST_ENCRYPTION_MASTER_KEY_BYTE: &str = "11";

pub fn block_on_secrets_init() -> Result<(), String> {
    // The bootstrap is async only for the Vault source; these fixtures use the
    // subprocess/env path, which never yields. A dedicated current-thread
    // runtime keeps the call legal from inside an existing tokio worker.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?
                    .block_on(SecretsBootstrap::try_init())
                    .map(|_s| ())
                    .map_err(|e| e.to_string())
            })
            .join()
            .unwrap_or_else(|_e| Err("secrets bootstrap thread panicked".to_owned()))
    })
}

// A provider without its api_key secret is withheld from the AI registry;
// the harness mocks every provider endpoint, so placeholder credentials let
// the mocked providers register.
/// Register one more named secret for the subprocess-mode secrets singleton.
///
/// Must run before the secrets bootstrap initialises: it appends `name` to
/// `SYSTEMPROMPT_CUSTOM_SECRETS` and exports the value under that name.
pub fn install_named_secret(name: &str, value: &str) {
    let mut custom = env::var("SYSTEMPROMPT_CUSTOM_SECRETS").unwrap_or_default();
    if !custom.split(',').any(|existing| existing == name) {
        if !custom.is_empty() {
            custom.push(',');
        }
        custom.push_str(name);
    }
    // SAFETY: called from single-threaded fixture init, before any thread
    // that reads the environment is spawned.
    unsafe {
        env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", custom);
        env::set_var(name, value);
    }
}

pub fn install_test_provider_keys() {
    for (name, value) in [
        ("ANTHROPIC_API_KEY", "test-anthropic-key"),
        ("OPENAI_API_KEY", "test-openai-key"),
        ("GEMINI_API_KEY", "test-gemini-key"),
    ] {
        if env::var(name).is_err() {
            // SAFETY: called from single-threaded fixture init, before any
            // thread that reads the environment is spawned.
            unsafe {
                env::set_var(name, value);
            }
        }
    }
}

pub fn ensure_test_secrets_bootstrap() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY: single-threaded test init; runs before any thread spawn.
        unsafe {
            env::set_var("SYSTEMPROMPT_SUBPROCESS", "1");
            if env::var("OAUTH_AT_REST_PEPPER").is_err() {
                env::set_var("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
            }
            if env::var("MANIFEST_SIGNING_SECRET_SEED").is_err() {
                env::set_var("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
            }
            install_test_provider_keys();
        }
        if env::var("encryption_master_key").is_err() {
            install_named_secret(
                "encryption_master_key",
                &TEST_ENCRYPTION_MASTER_KEY_BYTE.repeat(32),
            );
        }
        block_on_secrets_init().expect("SecretsBootstrap::try_init should succeed in tests");
    });
}
