//! [`TokenCacheKey`]: the one key a minted upstream token is cached under.
//!
//! A token is cached per catalog credential, never per provider, endpoint or
//! secret name: every endpoint a credential authenticates shares one minted
//! token, and two credentials never share one even when they name the same
//! secret. A personal credential's token is further keyed by the member it
//! belongs to, so one member's token is never sent on another's request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use systemprompt_identifiers::{CredentialId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TokenCacheKey(String);

impl TokenCacheKey {
    #[must_use]
    pub fn platform(credential: &CredentialId) -> Self {
        Self(format!("credential:{}", credential.as_str()))
    }

    #[must_use]
    pub fn personal(credential: &CredentialId, user: &UserId) -> Self {
        Self(format!(
            "credential:{}:user:{}",
            credential.as_str(),
            user.as_str()
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TokenCacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
