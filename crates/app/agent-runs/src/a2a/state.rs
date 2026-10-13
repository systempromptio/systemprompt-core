//! [`RunStatus`] ⇄ [`TaskState`].
//!
//! Every run status has a task state; the reverse is partial, since
//! `Rejected`, `InputRequired` and `AuthRequired` have no run equivalent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::a2a::TaskState;

use crate::outcome::RunStatus;

impl From<RunStatus> for TaskState {
    fn from(status: RunStatus) -> Self {
        match status {
            RunStatus::Queued => Self::Submitted,
            RunStatus::Preparing | RunStatus::Running => Self::Working,
            RunStatus::Succeeded => Self::Completed,
            RunStatus::Failed | RunStatus::TimedOut => Self::Failed,
            RunStatus::Cancelled => Self::Canceled,
        }
    }
}

impl RunStatus {
    #[must_use]
    pub const fn from_task_state(state: &TaskState) -> Option<Self> {
        match state {
            TaskState::Submitted => Some(Self::Queued),
            TaskState::Working => Some(Self::Running),
            TaskState::Completed => Some(Self::Succeeded),
            TaskState::Failed => Some(Self::Failed),
            TaskState::Canceled => Some(Self::Cancelled),
            TaskState::Rejected | TaskState::InputRequired | TaskState::AuthRequired => None,
        }
    }
}
