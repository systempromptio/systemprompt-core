//! Cross-command CLI utilities shared across command modules.
//!
//! Aggregates the command-result rendering sink, confirmation arguments,
//! setup database access, text helpers and the database-scoped command
//! error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod confirm;
pub mod ddl;
pub mod postgres_access;
pub mod render;
pub mod text;

pub use confirm::ConfirmArgs;
pub use render::render_result;
pub use text::truncate_with_ellipsis;

#[must_use]
pub fn database_scoped_command_error(
    host: &dyn systemprompt_extension::cli::CommandHost,
) -> anyhow::Error {
    host.profile().map_or_else(
        |_| {
            anyhow::anyhow!(
                "This command requires full profile context.\nPass --profile <local-profile> to \
                 target a local environment, or re-authenticate with 'systemprompt admin session \
                 login'."
            )
        },
        |profile| {
            anyhow::anyhow!(
                "Active profile '{}' routes to an external/cloud database, which this command does \
                 not support.\nPass --profile <local-profile> to target a local environment, or \
                 re-authenticate with 'systemprompt admin session login'.",
                profile.name
            )
        },
    )
}
