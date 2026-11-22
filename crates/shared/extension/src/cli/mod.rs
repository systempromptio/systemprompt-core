//! CLI command declarations and the types a command's body uses.
//!
//! [`KernelGrant`] and [`CommandClaim`] are always compiled, because the
//! kernel's reserved CLI paths name them. With the `cli` feature, a member
//! declares a clap group or leaf with `Contributions::command`, and the command
//! runs against a [`CommandHost`] — the one surface the CLI kernel exposes —
//! and returns a [`CommandOutput`] the kernel renders.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
mod admitted_views;
mod child;
mod claim;
#[cfg(feature = "cli")]
mod command;
#[cfg(feature = "cli")]
mod decl;
#[cfg(feature = "cli")]
mod descriptor;
mod grant;
#[cfg(feature = "cli")]
mod granted;
#[cfg(feature = "cli")]
mod impact;
#[cfg(feature = "cli")]
mod output;
#[cfg(feature = "cli")]
pub mod parsers;
#[cfg(feature = "cli")]
mod prompt;
#[cfg(feature = "cli")]
mod settings;
#[cfg(feature = "cli")]
pub mod time_format;

#[cfg(feature = "cli")]
pub use admitted_views::AdmittedViews;
pub use child::ChildSupervisor;
pub use claim::CommandClaim;
#[cfg(feature = "cli")]
pub use command::{
    BoxFuture, CliCommandGroup, CliCommandLeaf, CommandError, CommandFuture, CommandHost,
    CommandParent, DescribedCommand, REGISTERED_DISPLAY_ORDER, parents,
};
#[cfg(feature = "cli")]
pub use decl::{CommandDecl, CommandKind, ParsedCommand};
#[cfg(feature = "cli")]
pub use descriptor::{CommandDescriptor, DescribeCommand};
pub use grant::KernelGrant;
#[cfg(feature = "cli")]
pub use granted::{CallerSession, ServicesTree};
#[cfg(feature = "cli")]
pub use impact::{DataImpact, DbUrlRouting, RoutingClass};
#[cfg(feature = "cli")]
pub use output::{
    ChartType, CommandOutput, FileSummary, KeyValueItem, KeyValueOutput, SuccessOutput,
    TableOutput, TextOutput,
};
#[cfg(feature = "cli")]
pub use prompt::{
    ConfirmationRefused, FlagRequired, Prompter, require_confirmation,
    require_confirmation_default_yes, resolve_required,
};
#[cfg(feature = "cli")]
pub use settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
