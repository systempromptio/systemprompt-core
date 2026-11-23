//! The deployment's OAuth at-rest pepper as a kernel seed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

/// The `oauth_at_rest_pepper` secret the OAuth grant tables digest their
/// codes and tokens under. The host seeds it sealed; only the `oauth`
/// member's provide reads it, to build its repositories.
#[derive(Clone)]
pub struct OAuthAtRestPepper(Arc<str>);

impl OAuthAtRestPepper {
    #[must_use]
    pub fn new(pepper: &str) -> Self {
        Self(Arc::from(pepper))
    }

    #[must_use]
    pub fn shared(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

impl std::fmt::Debug for OAuthAtRestPepper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OAuthAtRestPepper(<redacted>)")
    }
}
