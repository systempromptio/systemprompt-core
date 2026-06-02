//! Cross-command CLI utilities shared across command modules.
//!
//! Aggregates the [`CommandOutput`] artifact model and its output types,
//! profile resolution ([`resolve_profile_path`],
//! [`resolve_profile_with_data`]), argument parsers, and text helpers. Also
//! defines the `define_pool_command!` macro used by the log commands to
//! generate pooled execution entry points.

pub mod command_result;
pub mod docker;
pub mod parsers;
pub mod paths;
pub mod process;
pub mod profile;
pub mod project;
pub mod text;

pub use command_result::{
    ChartType, CommandOutput, KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput, TextOutput,
    render_result,
};
pub use parsers::{parse_email, parse_profile_name};
pub use profile::{
    ProfileResolutionError, is_path_input, resolve_profile_from_path, resolve_profile_path,
    resolve_profile_with_data,
};
pub use text::truncate_with_ellipsis;

#[macro_export]
macro_rules! define_pool_command {
    ($args_ty:ty => $ret_ty:ty, with_config) => {
        pub(in $crate::commands::infrastructure::logs) async fn execute(
            args: $args_ty,
            config: &$crate::CliConfig,
        ) -> ::anyhow::Result<$ret_ty> {
            let ctx = ::systemprompt_runtime::AppContext::new().await?;
            let pool = ctx.db_pool().pool_arc()?;
            execute_with_pool_inner(args, &pool, config).await
        }

        pub(in $crate::commands::infrastructure::logs) async fn execute_with_pool(
            args: $args_ty,
            db_ctx: &::systemprompt_runtime::DatabaseContext,
            config: &$crate::CliConfig,
        ) -> ::anyhow::Result<$ret_ty> {
            let pool = db_ctx.db_pool().pool_arc()?;
            execute_with_pool_inner(args, &pool, config).await
        }
    };
    ($args_ty:ty => $ret_ty:ty, no_config) => {
        pub(in $crate::commands::infrastructure::logs) async fn execute(
            args: $args_ty,
            _config: &$crate::CliConfig,
        ) -> ::anyhow::Result<$ret_ty> {
            let ctx = ::systemprompt_runtime::AppContext::new().await?;
            let pool = ctx.db_pool().pool_arc()?;
            execute_with_pool_inner(args, &pool).await
        }

        pub(in $crate::commands::infrastructure::logs) async fn execute_with_pool(
            args: $args_ty,
            db_ctx: &::systemprompt_runtime::DatabaseContext,
            _config: &$crate::CliConfig,
        ) -> ::anyhow::Result<$ret_ty> {
            let pool = db_ctx.db_pool().pool_arc()?;
            execute_with_pool_inner(args, &pool).await
        }
    };
}
