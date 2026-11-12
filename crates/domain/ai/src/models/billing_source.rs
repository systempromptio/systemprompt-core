//! [`BillingSource`]: who pays for a request, read off its credential.
//!
//! `ai_requests.billing_source` is `platform` for a request sent with a
//! deployment credential, `user_key` for one sent with a member's own stored
//! key and `user_subscription` for one sent with the credential the client
//! presented on the request. It is derived from the catalog credential's kind
//! and nothing else, so a request is billed to a member exactly when the
//! credential that authenticated it is the member's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::services::catalog::{CredentialKind, ResolvedCredential};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingSource {
    Platform,
    UserKey,
    UserSubscription,
}

impl BillingSource {
    #[must_use]
    pub const fn from_kind(kind: CredentialKind) -> Self {
        match kind {
            CredentialKind::Platform => Self::Platform,
            CredentialKind::Personal => Self::UserKey,
            CredentialKind::Caller => Self::UserSubscription,
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
            Self::UserSubscription => "user_subscription",
        }
    }

    #[must_use]
    pub const fn is_platform(self) -> bool {
        match self {
            Self::Platform => true,
            Self::UserKey | Self::UserSubscription => false,
        }
    }
}
