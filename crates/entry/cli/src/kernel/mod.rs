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
//!   session, foreground children) and the output settings in [`CliConfig`] and
//!   [`OutputFormat`].
//! - **Output** — [`CommandOutput`] and its row/section types, rendered with
//!   [`render_result`]; validation results in [`ValidationOutput`].
//! - **Operator interaction** — [`Prompter`], [`require_confirmation`] and
//!   [`ConfirmArgs`] for `--yes`.
//! - **Argument parsing** — the typed-identifier value parsers in [`parsers`].
//! - **Profile access** — [`resolve_profile_path`], [`profile`], [`boot`
//!   loading](load_boot), [`ResolvedPaths`], [`project`] discovery and the
//!   database-scope refusal [`database_scoped_command_error`].
//! - **Session** — [`session`], [`CliSessionContext`] and
//!   [`get_or_create_session`] for commands that call the API as the operator.
//! - **Presentation and process** — [`tables`], [`StartupRenderer`],
//!   [`time_format`], [`text`], [`disk_logs`], [`interrupt`] and [`routing`],
//!   plus [`define_pool_command!`] for pooled database commands.
//! - **Helpers several groups share** — [`provider_catalog`] (the seeded model
//!   catalog and `gateway.yaml`), [`docker_compose`] and [`postgres_access`]
//!   (the local database), [`yaml_document`] (lossless YAML edits),
//!   [`ProfileInfo`] and [`McpToolEntry`]. A command group names these here and
//!   never reaches into another group's modules.
//!
//! Errors are `anyhow::Result<()>` from `run`; the runner maps them to the
//! process exit code, and an interrupted command exits with
//! [`INTERRUPTED_EXIT_CODE`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod registry;

pub use registry::{
    CliCommandGroup, CliCommandLeaf, CliCommandRegistration, CommandFuture, CommandRegistry,
    RegisteredCommand, RegistryError,
};

pub use crate::boot::{load_boot, load_boot_from_path, load_profile};
pub use crate::cli_settings::{CliConfig, OutputFormat};
pub use crate::commands::shared::mcp_tools::McpToolEntry;
pub use crate::commands::shared::{ValidationIssue, ValidationOutput, mcp_tools};
pub use crate::context::CommandContext;
pub use crate::define_pool_command;
pub use crate::descriptor::{CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand};
pub use crate::interactive::{
    DialoguerPrompter, Prompter, require_confirmation, require_confirmation_default_yes,
    resolve_required,
};
pub use crate::paths::ResolvedPaths;
pub use crate::presentation::{StartupRenderer, tables};
pub use crate::runner::interrupt::INTERRUPTED_EXIT_CODE;
pub use crate::runner::{interrupt, routing};
pub use crate::session::{self, CliSessionContext, get_or_create_session};
pub use crate::shared::command_result::{
    ChartType, CommandOutput, KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput, TextOutput,
    render_result,
};
pub use crate::shared::profile::ProfileInfo;
pub use crate::shared::{
    ConfirmArgs, IdentityBundle, database_scoped_command_error, disk_logs, docker_compose,
    generate_identity, parsers, postgres_access, profile, project, provider_catalog,
    resolve_profile_path, text, time_format, yaml_document,
};
