//! [`BillingSource`]: who pays for a request, read off its credential.
//!
//! `ai_requests.billing_source` is `platform` for a request sent with a
//! deployment credential and `user_key` for one sent with a member's own key.
//! It is derived from the catalog credential's kind and nothing else, so a
//! request is billed to a personal subscription exactly when the credential
//! that authenticated it is personal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::catalog::{CredentialKind, ResolvedCredential};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingSource {
    Platform,
    UserKey,
}

impl BillingSource {
    #[must_use]
    pub const fn from_kind(kind: CredentialKind) -> Self {
        match kind {
            CredentialKind::Platform => Self::Platform,
            CredentialKind::Personal => Self::UserKey,
        }
    }

    #[must_use]
    pub const fn of(credential: &ResolvedCredential) -> Self {
        Self::from_kind(credential.kind)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::UserKey => "user_key",
        }
    }
}
