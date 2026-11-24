//! The identity capability's commands: `admin users`, `admin keys` and
//! `admin bridge`, contributed by
//! [`crate::extension::IdentityHttpExtension`].
//!
//! Each group reaches kernel-held material only through the grants its
//! reserved CLI path lists (`config-plane-writer` on `admin users`,
//! `signing-keys` on `admin keys` and `admin bridge`), the values
//! conferred on `identity-http` (the OAuth repositories and the owner
//! reassignments) and the kernel-built
//! [`IdentityCliSettings`](systemprompt_extension::cli::IdentityCliSettings).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bridge;
pub mod keys;
pub mod users;

use systemprompt_extension::cli::CommandError;

pub(crate) fn command_error(error: anyhow::Error) -> CommandError {
    match error.downcast::<CommandError>() {
        Ok(failed) => failed,
        Err(error) => CommandError::new(error),
    }
}
