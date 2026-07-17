//! `BearerToken` wrapper that redacts itself in `Debug`/`Display` output.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::BearerToken;

pub type Secret = BearerToken;

impl BearerToken {
    #[must_use]
    pub fn expose(&self) -> &str {
        self.as_str()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.as_str().len()
    }
}

impl Default for BearerToken {
    fn default() -> Self {
        Self::new(String::new())
    }
}
