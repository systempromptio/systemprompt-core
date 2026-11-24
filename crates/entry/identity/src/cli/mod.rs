//! The identity administration commands: `admin users`, `admin bridge` and
//! `admin keys`.
//!
//! `identity-http` contributes the three groups under the kernel's `admin`
//! group (see [`crate::extension`]). `users` holds the identity plane-writer
//! grant and is served on a `--database-url` invocation for its read-only
//! commands; `bridge` and `keys` hold the signing-keys grant, reach the
//! deployment's signing state only through the kernel-held
//! [`DeploymentSigningKeys`](systemprompt_extension::cli::DeploymentSigningKeys)
//! handle and need a full profile context.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bridge;
pub mod keys;
pub(crate) mod state;
pub mod users;

use systemprompt_extension::cli::CommandError;

fn command_error(error: anyhow::Error) -> CommandError {
    match error.downcast::<CommandError>() {
        Ok(failed) => failed,
        Err(error) => CommandError::new(error),
    }
}
