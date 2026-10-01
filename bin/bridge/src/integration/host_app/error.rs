//! The typed failure of a `HostApp` operation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

use crate::host_sync::ForeignShape;
use crate::install::approval::{ApprovalRefusal, ElevationFailure, GatedChangeError};

/// Why a host's generate, install, remove or open did not complete.
///
/// `Declined` and `NeedsPrompt` are refusals, not failures: the change was not
/// made because approval was withheld. Every other variant is a failure, so an
/// operating-system denial (`Io` with `PermissionDenied`) is never reported as
/// a decline.
#[derive(Debug, thiserror::Error)]
pub enum HostAppError {
    #[error("administrator approval was declined; the change was not made")]
    Declined,
    #[error("{reason}")]
    NeedsPrompt { reason: &'static str },
    #[error(transparent)]
    ForeignShape(#[from] ForeignShape),
    #[error(transparent)]
    Elevation(#[from] ElevationFailure),
    #[error("this agent cannot be opened from the bridge")]
    OpenUnsupported,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl HostAppError {
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Declined | Self::NeedsPrompt { .. })
    }

    #[must_use]
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, Self::Io(e) if e.kind() == io::ErrorKind::PermissionDenied)
    }
}

impl From<ApprovalRefusal> for HostAppError {
    fn from(refusal: ApprovalRefusal) -> Self {
        match refusal {
            ApprovalRefusal::Declined => Self::Declined,
            ApprovalRefusal::NeedsPrompt { reason } => Self::NeedsPrompt { reason },
        }
    }
}

impl From<GatedChangeError> for HostAppError {
    fn from(error: GatedChangeError) -> Self {
        match error {
            GatedChangeError::Refused(refusal) => refusal.into(),
            GatedChangeError::Elevation(failure) => Self::Elevation(failure),
            GatedChangeError::Io(e) => Self::Io(e),
        }
    }
}
