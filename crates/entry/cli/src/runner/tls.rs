//! The process-wide rustls crypto provider, installed before any TLS client
//! is built.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// Why: reqwest 0.13 is built with `rustls-no-provider` so the tree carries ring
// only (sqlx already uses it); rustls then needs a process-wide provider before
// the first client is built, or reqwest panics.
pub(super) fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return;
    }
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("rustls crypto provider was installed concurrently");
    }
}
