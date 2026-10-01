//! A change the user (or the unattended context) did not approve.
//!
//! An install that stops because administrator approval was declined, or
//! because an unattended run may not raise the prompt it needs, is not a
//! failed write. The refusal travels inside the `io::Error` the host
//! installers return and is recovered by type, never by error kind or
//! message text, so an operating-system denial still reads as a failure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

/// Why an approval-gated change did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalRefusal {
    #[error("administrator approval was declined; the change was not made")]
    Declined,
    #[error("{reason}")]
    NeedsPrompt { reason: &'static str },
}

impl ApprovalRefusal {
    #[must_use]
    pub fn of(error: &io::Error) -> Option<Self> {
        error.get_ref()?.downcast_ref::<Self>().copied()
    }
}

impl From<ApprovalRefusal> for io::Error {
    fn from(refusal: ApprovalRefusal) -> Self {
        Self::new(io::ErrorKind::PermissionDenied, refusal)
    }
}
