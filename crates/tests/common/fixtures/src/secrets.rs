//! Shared secrets helpers for integration tests.
//!
//! Every test that touches the JWT/OAuth/security path takes its secrets from
//! [`ensure_test_secrets_bootstrap`], which is the secrets service of the
//! process-wide fixture bootstrap. The service loads through the subprocess
//! path, so it matches how production deployments load deployment secrets in
//! air-gapped / container modes.

use std::env;
use std::sync::Arc;

use systemprompt_config::{LoadedProfile, SecretsService};

pub fn block_on_secrets_load(profile: &Arc<LoadedProfile>) -> Result<Arc<SecretsService>, String> {
    // The bootstrap is async only for the Vault source; these fixtures use the
    // env source, which never yields. A dedicated current-thread
    // runtime keeps the call legal from inside an existing tokio worker.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?
                    .block_on(SecretsService::load(
                        Arc::clone(profile),
                        systemprompt_config::SecretsSources::default(),
                    ))
                    .map_err(|e| e.to_string())
            })
            .join()
            .unwrap_or_else(|_e| Err("secrets bootstrap thread panicked".to_owned()))
    })
}

// A provider without its api_key secret is withheld from the AI registry;
// the harness mocks every provider endpoint, so placeholder credentials let
// the mocked providers register.
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

pub fn ensure_test_secrets_bootstrap() -> Arc<SecretsService> {
    Arc::clone(crate::bootstrap::ensure_test_bootstrap().boot.secrets())
}

// The `ai` member's at-rest cipher (the owner of `user_provider_credentials`)
// under the bootstrap secrets' master key.
pub fn fixture_at_rest_cipher() -> systemprompt_security::at_rest::AtRestCipher {
    systemprompt_security::at_rest::AtRestCipher::for_member(
        &fixture_master_key(),
        &systemprompt_identifiers::ExtensionId::new("ai"),
    )
    .expect("the bootstrap master key keys an HMAC")
}

pub fn fixture_master_key() -> [u8; 32] {
    let secrets = ensure_test_secrets_bootstrap().get();
    let encoded = secrets
        .get("encryption_master_key")
        .expect("the bootstrap secrets carry an encryption master key");
    systemprompt_config::decode_master_key(encoded).expect("a 32-byte hex master key")
}
