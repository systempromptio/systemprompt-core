//! The agent capability's commands: `admin agents`, `core contexts` and
//! `core artifacts`, contributed by
//! [`crate::extension::AgentHttpExtension`].
//!
//! Each group reaches kernel-held material only through the grants its
//! reserved CLI path lists and the open values the agent member provides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod agents;
pub mod artifacts;
pub mod contexts;

use systemprompt_extension::cli::CommandError;

pub(crate) fn command_error(error: anyhow::Error) -> CommandError {
    match error.downcast::<CommandError>() {
        Ok(failed) => failed,
        Err(error) => CommandError::new(error),
    }
}
