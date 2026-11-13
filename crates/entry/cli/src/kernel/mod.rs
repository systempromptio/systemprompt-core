//! The CLI kernel surface command groups build on.
//!
//! A command group, whether it lives in this crate or in a capability's entry
//! crate, reaches the kernel only through this module:
//!
//! - **Registration** — [`CliCommandGroup`], [`register_cli_command!`] and the
//!   [`CommandRegistry`] the runner assembles the command tree from.
//! - **Bootstrap declaration** — [`CommandDescriptor`], [`DataImpact`],
//!   [`DbUrlRouting`] and [`DescribeCommand`]: what a command needs before it
//!   runs and what it does to the data behind the profile.
//! - **Execution context** — [`CommandContext`] (profile, paths, database,
//!   session, foreground children) and the output settings in [`CliConfig`]
//!   and [`OutputFormat`].
//! - **Output** — [`CommandOutput`] and its row/section types, rendered with
//!   [`render_result`]; validation results in [`ValidationOutput`].
//! - **Operator interaction** — [`Prompter`], [`require_confirmation`] and
//!   [`ConfirmArgs`] for `--yes`.
//! - **Argument parsing** — the typed-identifier value parsers in
//!   [`parsers`].
//! - **Profile access** — [`resolve_profile_path`] and the database-scope
//!   refusal [`database_scoped_command_error`].
//!
//! Errors are `anyhow::Result<()>` from `run`; the runner maps them to the
//! process exit code, and an interrupted command exits with
//! [`INTERRUPTED_EXIT_CODE`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod registry;

pub use registry::{
    CliCommandGroup, CliCommandRegistration, CommandFuture, CommandRegistry, RegisteredCommand,
    RegistryError,
};

pub use crate::cli_settings::{CliConfig, OutputFormat};
pub use crate::commands::shared::{ValidationIssue, ValidationOutput};
pub use crate::context::CommandContext;
pub use crate::descriptor::{CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand};
pub use crate::interactive::{Prompter, require_confirmation};
pub use crate::runner::interrupt::INTERRUPTED_EXIT_CODE;
pub use crate::shared::command_result::{
    ChartType, CommandOutput, KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput, TextOutput,
    render_result,
};
pub use crate::shared::{
    ConfirmArgs, database_scoped_command_error, parsers, resolve_profile_path,
};
