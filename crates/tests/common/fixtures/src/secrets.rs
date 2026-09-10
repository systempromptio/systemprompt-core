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
        }
        block_on_secrets_init().expect("SecretsBootstrap::try_init should succeed in tests");
    });
}
