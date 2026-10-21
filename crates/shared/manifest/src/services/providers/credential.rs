//! Whose credential serves a provider: the deployment's or the caller's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Whose credential serves a provider.
///
/// `platform` providers are served from the deployment secret named by
/// `api_key_secret`. `user` providers are served only from a key the calling
/// user stored for themselves (a personal subscription), never from a
/// deployment secret, so one person's subscription is never spent on another.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialSource {
    #[default]
    Platform,
    User,
}

impl ProviderCredentialSource {
    #[must_use]
    pub const fn is_platform(&self) -> bool {
        matches!(self, Self::Platform)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::User => "user",
        }
    }
}
