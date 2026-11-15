//! Cross-command CLI utilities shared across command modules.
//!
//! Aggregates the [`CommandOutput`] artifact model and its output types,
//! profile resolution ([`resolve_profile_path`],
//! [`resolve_profile_with_data`]), argument parsers, and text helpers. Also
//! defines the `define_pool_command!` macro used by the log commands to
//! generate pooled execution entry points.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod confirm;
pub mod ddl;
pub mod disk_logs;
pub mod docker_compose;
pub mod postgres_access;
pub mod profile;
pub mod render;
pub mod text;

pub use render::render_result;
pub use confirm::ConfirmArgs;
pub use profile::{
    ProfileResolutionError, ProfileSource, ResolvedProfile, is_path_input,
    resolve_profile_from_path, resolve_profile_path, resolve_profile_with_data,
};
pub use text::truncate_with_ellipsis;

#[must_use]
pub fn database_scoped_command_error(ctx: &crate::context::CommandContext) -> anyhow::Error {
    ctx.profile().map_or_else(
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

#[macro_export]
macro_rules! define_pool_command {
    ($args_ty:ty => $ret_ty:ty, with_config) => {
        pub(in $crate::commands::infrastructure::logs) async fn execute(
            args: $args_ty,
            ctx: &$crate::context::CommandContext,
        ) -> ::anyhow::Result<$ret_ty> {
            let pool = ctx.db_pool().await?.pool();
            execute_with_pool_inner(args, &pool, &ctx.cli).await
        }
    };
    ($args_ty:ty => $ret_ty:ty, no_config) => {
        pub(in $crate::commands::infrastructure::logs) async fn execute(
            args: $args_ty,
            ctx: &$crate::context::CommandContext,
        ) -> ::anyhow::Result<$ret_ty> {
            let pool = ctx.db_pool().await?.pool();
            execute_with_pool_inner(args, &pool).await
        }
    };
}
