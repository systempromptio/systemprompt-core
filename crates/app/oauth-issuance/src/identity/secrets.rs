//! [`IdentityClientSecrets`]: the identity member's narrow view of the secret
//! store, built in its provide fn from the sealed store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_traits::{ExtensionSecret, SecretStore};

/// The one secret-derived capability the identity plane holds: a declared
/// OAuth client's custom secret, by the key its declaration names.
#[derive(Clone)]
pub struct IdentityClientSecrets {
    store: Arc<dyn SecretStore>,
}

impl IdentityClientSecrets {
    #[must_use]
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self { store }
    }

    pub(crate) fn client_secret(&self, key: &str) -> Option<ExtensionSecret> {
        self.store.secret(key)
    }
}

impl std::fmt::Debug for IdentityClientSecrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityClientSecrets")
            .finish_non_exhaustive()
    }
}
