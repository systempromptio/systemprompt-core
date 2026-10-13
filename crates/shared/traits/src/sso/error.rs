//! SSO failure taxonomy and its two stable projections: the coarse
//! `reason_code` shown to the browser, and the finer `audit_reason` recorded
//! against the sign-in.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::BoxedSource;

use super::SsoRefusal;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SsoError {
    #[error("SSO provider unavailable: {0}")]
    Unavailable(String),
    #[error("SSO state missing, expired or already used")]
    StateLost,
    #[error("Unsolicited SSO response not allowed")]
    UnsolicitedNotAllowed,
    #[error("Invalid SSO assertion: {0}")]
    InvalidAssertion(String),
    #[error("SSO assertion replayed")]
    Replayed,
    #[error("SSO assertion carries no subject")]
    NoSubject,
    #[error("SSO assertion carries no email")]
    NoEmail,
    #[error("SSO sign-in refused: {0:?}")]
    Refused(SsoRefusal),
    #[error("SSO internal error: {0}")]
    Internal(#[source] BoxedSource),
}

impl SsoError {
    pub fn internal(source: impl Into<BoxedSource>) -> Self {
        Self::Internal(source.into())
    }

    #[must_use]
    pub const fn reason_code(&self) -> &'static str {
        match self {
            Self::Unavailable(_) => "unavailable",
            Self::Internal(_) => "error",
            Self::NoSubject => "no_subject",
            Self::StateLost | Self::UnsolicitedNotAllowed => "correlation_lost",
            Self::InvalidAssertion(_) | Self::Replayed => "invalid_assertion",
            Self::NoEmail => "no_email",
            Self::Refused(SsoRefusal::DomainNotAllowed) => "forbidden",
            Self::Refused(SsoRefusal::GroupOverage) => "group_overage",
            Self::Refused(SsoRefusal::NoGroup) => "no_group",
            Self::Refused(SsoRefusal::NotProvisioned) => "not_provisioned",
        }
    }

    #[must_use]
    pub const fn audit_reason(&self) -> &'static str {
        match self {
            Self::Replayed => "replayed",
            Self::UnsolicitedNotAllowed => "unsolicited",
            Self::StateLost => "state_lost",
            other => other.reason_code(),
        }
    }
}

impl From<SsoRefusal> for SsoError {
    fn from(refusal: SsoRefusal) -> Self {
        Self::Refused(refusal)
    }
}
