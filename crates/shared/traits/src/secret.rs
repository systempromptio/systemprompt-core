//! [`ExtensionSecret`]: one operator-provisioned secret handed to an
//! extension by name.
//!
//! Extensions read their credentials (an encryption master key, a webhook
//! signing secret) from the platform secrets document through the extension
//! context instead of the process environment, so a secret follows the
//! configured secrets source and its reloads. The value redacts itself in
//! `Debug` and is wiped from memory on drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use zeroize::Zeroize;

pub struct ExtensionSecret {
    value: String,
}

impl ExtensionSecret {
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self { value }
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for ExtensionSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExtensionSecret(<redacted>)")
    }
}

impl Drop for ExtensionSecret {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}
