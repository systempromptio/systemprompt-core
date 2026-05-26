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
        SecretsBootstrap::try_init().expect("SecretsBootstrap::try_init should succeed in tests");
    });
}
