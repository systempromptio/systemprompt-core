//! The status a headless command ends with, and the process exit code each
//! one maps to.
//!
//! `0` completed, `1` the session ended with an error, `5` no signed-in
//! account, `10` the instance could not be reached, `64` the arguments were
//! wrong, `70` the composition could not be started or failed internally,
//! `77` the instance or its policy refused, `124` the run outlived its
//! limit, `130` the run was aborted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
use systemprompt_device::view::{SignInFailure, SyncErrorKind};
use systemprompt_engine::run::ErrorKind;
use systemprompt_engine::store::MessageErrorKind;

/// How a headless command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitStatus {
    Completed,
    Failed,
    NotSignedIn,
    Unavailable,
    Usage,
    Internal,
    Refused,
    TimedOut,
    Aborted,
}

impl ExitStatus {
    pub const fn code(self) -> u8 {
        match self {
            Self::Completed => 0,
            Self::Failed => 1,
            Self::NotSignedIn => 5,
            Self::Unavailable => 10,
            Self::Usage => 64,
            Self::Internal => 70,
            Self::Refused => 77,
            Self::TimedOut => 124,
            Self::Aborted => 130,
        }
    }

    pub const fn is_success(self) -> bool {
        matches!(self, Self::Completed)
    }

    pub const fn of_engine(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::NotSignedIn => Self::NotSignedIn,
            ErrorKind::ModelNotOffered | ErrorKind::Governance => Self::Refused,
            ErrorKind::InvalidInput => Self::Usage,
            ErrorKind::Unavailable => Self::Unavailable,
            ErrorKind::Aborted => Self::Aborted,
            ErrorKind::Model | ErrorKind::ContextOverflow => Self::Failed,
            ErrorKind::NotFound
            | ErrorKind::Busy
            | ErrorKind::Config
            | ErrorKind::Storage
            | ErrorKind::Workspace
            | ErrorKind::Internal => Self::Internal,
        }
    }

    pub const fn of_message(kind: MessageErrorKind) -> Self {
        match kind {
            MessageErrorKind::Aborted => Self::Aborted,
            MessageErrorKind::Auth => Self::NotSignedIn,
            MessageErrorKind::Governance => Self::Refused,
            MessageErrorKind::OutputLength
            | MessageErrorKind::ContextOverflow
            | MessageErrorKind::ContentFilter
            | MessageErrorKind::Api
            | MessageErrorKind::Unknown => Self::Failed,
        }
    }

    pub const fn of_sync(kind: SyncErrorKind) -> Self {
        match kind {
            SyncErrorKind::Unreachable | SyncErrorKind::RateLimited => Self::Unavailable,
            SyncErrorKind::Refused | SyncErrorKind::KeyChanged | SyncErrorKind::Invalid => {
                Self::Refused
            },
            SyncErrorKind::Insecure => Self::NotSignedIn,
            SyncErrorKind::Service => Self::Internal,
        }
    }

    pub const fn of_sign_in(failure: SignInFailure) -> Self {
        match failure {
            SignInFailure::Unreachable | SignInFailure::RateLimited => Self::Unavailable,
            SignInFailure::Refused => Self::Refused,
            SignInFailure::Incompatible | SignInFailure::Insecure | SignInFailure::Failed => {
                Self::Failed
            },
        }
    }
}

impl From<ExitStatus> for std::process::ExitCode {
    fn from(status: ExitStatus) -> Self {
        Self::from(status.code())
    }
}
