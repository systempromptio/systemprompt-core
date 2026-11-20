//! The CLI kernel: the command registry, command-line description and the
//! helpers the kernel's own groups share.
//!
//! A contributed command declares against `systemprompt_extension::cli`
//! (`CommandDecl`, `CliCommandGroup`, `CliCommandLeaf`, `CommandHost`,
//! `CommandOutput`) and never against this module. The kernel builds its
//! [`CommandRegistry`] from the composition's declared commands and its own
//! built-ins, re-checking every reserved path and grant;
//! [`describe_command_line`] classifies a forwarded argv against that
//! registry, [`command_line_path`] names the path it selects and
//! [`classify_command_line`] returns both with the owner, refusing a
//! disabled owner's command. [`read_disabled`] reads the one profile input
//! admission needs from a target profile the host does not run. Output is
//! rendered by [`render_result`]; the remaining items are the helpers the
//! kernel's own groups share (profile access, session, presentation, the
//! local database and the seeded model catalog). An interrupted command
//! exits with [`INTERRUPTED_EXIT_CODE`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod describe;
mod registry;
mod registry_error;
mod target_admission;

pub use describe::{classify_command_line, command_line_path, describe_command_line};
pub use registry::{
    CommandOwner, CommandRegistry, RegistryEntry, ResolvedCommand, kernel_commands,
};
pub use registry_error::{RegistryError, ResolveError};
pub(crate) use target_admission::admit_target;
pub use target_admission::{TargetAdmissionError, read_disabled};

pub use crate::commands::shared::mcp_tools::McpToolEntry;
pub use crate::commands::shared::{ValidationIssue, ValidationOutput, mcp_tools};
pub use crate::context::CommandContext;
pub use crate::host::KernelHost;
pub use crate::member_host::MemberHost;
pub use crate::presentation::{StartupRenderer, tables};
pub use crate::runner::interrupt::INTERRUPTED_EXIT_CODE;
pub use crate::runner::{interrupt, routing};
pub use crate::session::{self, CliSessionContext, get_or_create_session};
pub use crate::shared::render::render_result;
pub use crate::shared::{
    ConfirmArgs, database_scoped_command_error, ddl, disk_logs, postgres_access, text,
};
