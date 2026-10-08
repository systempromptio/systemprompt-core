//! A change the user (or the unattended context) did not approve, and the
//! typed failure of an approval-gated change.
//!
//! An install that stops because administrator approval was declined, or
//! because an unattended run may not raise the prompt it needs, is not a
//! failed write. Each outcome is its own variant of [`GatedChangeError`], so an
//! operating-system denial stays an `Io` failure and a refusal is never
//! recovered from an error kind or message text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use super::elevated_protocol::ProtocolError;

/// Why an approval-gated change did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalRefusal {
    #[error("administrator approval was declined; the change was not made")]
    Declined,
    #[error("{reason}")]
    NeedsPrompt { reason: &'static str },
}

/// The elevated step ran (or tried to) and its result cannot be accepted.
#[derive(Debug, thiserror::Error)]
pub enum ElevationFailure {
    #[error("the elevated helper could not run: {0}")]
    Helper(String),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("the elevated change did not land: {0}")]
    Unverified(String),
    #[cfg(target_os = "macos")]
    #[error(transparent)]
    Privileged(super::elevate::ElevationError),
}

/// The failure of a change that may need administrator approval.
#[derive(Debug, thiserror::Error)]
pub enum GatedChangeError {
    #[error(transparent)]
    Refused(#[from] ApprovalRefusal),
    #[error(transparent)]
    Elevation(#[from] ElevationFailure),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl GatedChangeError {
    #[must_use]
    pub const fn refusal(&self) -> Option<ApprovalRefusal> {
        match self {
            Self::Refused(refusal) => Some(*refusal),
            Self::Elevation(_) | Self::Io(_) => None,
        }
    }
}
