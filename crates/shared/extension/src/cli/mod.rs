//! CLI command declarations and the types a command's body uses.
//!
//! [`KernelGrant`] is always compiled, because the kernel's reserved CLI
//! paths name it. With the `cli` feature, a member declares a clap group or
//! leaf with `Contributions::command`, and the command runs against a
//! [`CommandHost`] — the one surface the CLI kernel exposes — and returns a
//! [`CommandOutput`] the kernel renders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
mod command;
#[cfg(feature = "cli")]
mod descriptor;
mod grant;
#[cfg(feature = "cli")]
mod output;
#[cfg(feature = "cli")]
mod prompt;
#[cfg(feature = "cli")]
mod settings;

#[cfg(feature = "cli")]
pub use command::{
    BoxFuture, ChildSupervisor, CliCommandGroup, CliCommandLeaf, CommandDecl, CommandError,
    CommandFuture, CommandHost, CommandKind, CommandParent, ParsedCommand,
    REGISTERED_DISPLAY_ORDER, parents,
};
#[cfg(feature = "cli")]
pub use descriptor::{CommandDescriptor, DataImpact, DbUrlRouting, DescribeCommand, RoutingClass};
pub use grant::KernelGrant;
#[cfg(feature = "cli")]
pub use output::{
    ChartType, CommandOutput, KeyValueItem, KeyValueOutput, SuccessOutput, TableOutput,
    TextOutput,
};
#[cfg(feature = "cli")]
pub use prompt::{
    ConfirmationRefused, Prompter, require_confirmation, require_confirmation_default_yes,
};
#[cfg(feature = "cli")]
pub use settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
