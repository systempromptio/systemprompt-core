//! The process-wide cryptography provider the HTTP client needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: the HTTP client is built without a cryptography provider, so one must
// be installed before the first client is built or the build panics.
pub(crate) fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return;
    }
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("the cryptography provider was installed concurrently");
    }
}
