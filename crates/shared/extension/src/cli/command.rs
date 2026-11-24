//! Command declarations, the traits a command implements, and the host it
//! runs against.
//!
//! A member declares a clap [`CliCommandGroup`] or [`CliCommandLeaf`] with
//! `Contributions::command(CommandDecl::…)`, naming the [`CommandParent`] it
//! attaches under and the [`KernelGrant`]s it requests. The declaration is
//! type-erased: it carries the clap command builder and a resolver that
//! parses matches into a [`ParsedCommand`], whose future runs the command
//! against a [`CommandHost`] and returns its [`CommandOutput`]. The CLI kernel
//! renders the output; a command never prints.
//!
//! `DISPLAY_ORDER` places a top-level group in `--help`. The kernel's own
//! groups hold the slots `10` (`core`), `20` (`infra`), `30` (`admin`), `60`
//! (`web`), `70` (`plugins`) and `80` (`build`); `40` and `50` are the `cloud`
//! and `analytics` slots. A declaration that names no slot gets
//! [`REGISTERED_DISPLAY_ORDER`]; the registry lists such top-level groups from
//! that slot on in composition order, after every kernel group and before
//! clap's `help` (slot `999`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use clap::{Args, Subcommand};
use systemprompt_identifiers::{ContextId, ExtensionId};
use systemprompt_manifest::{Config, Profile};
use systemprompt_traits::DatabaseHandle;

use super::admitted_views::AdmittedViews;
use super::child::ChildSupervisor;
use super::descriptor::{CommandDescriptor, DescribeCommand};
use super::grant::GrantedPlane;
use super::granted::{CallerSession, DeploymentSigningKeys, IdentityCliSettings, ServicesTree};
use super::output::CommandOutput;
use super::prompt::Prompter;
use super::settings::CliConfig;
use crate::contribute::Resolved;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type CommandFuture<'a> = BoxFuture<'a, Result<CommandOutput, CommandError>>;

pub const REGISTERED_DISPLAY_ORDER: usize = 900;

/// A failed command: the cause, and optionally the output it produced
/// before failing (a validation report, say), which the CLI kernel renders
/// once before it reports the failure and exits non-zero.
#[derive(Debug)]
pub struct CommandError {
    source: Box<dyn std::error::Error + Send + Sync>,
    output: Option<Box<CommandOutput>>,
}

impl CommandError {
    pub fn new(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: error.into(),
            output: None,
        }
    }

    #[must_use]
    pub fn with_output(mut self, output: CommandOutput) -> Self {
        self.output = Some(Box::new(output));
        self
    }

    #[must_use]
    pub fn output(&self) -> Option<&CommandOutput> {
        self.output.as_deref()
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CommandError {
    fn from(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::new(source)
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, f)
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

/// A kernel group a contributed command attaches under.
pub trait CommandParent: 'static {
    const PATH: &'static [&'static str];
}

pub mod parents {
    use super::CommandParent;

    #[derive(Debug, Clone, Copy)]
    pub struct Core;
    #[derive(Debug, Clone, Copy)]
    pub struct Infra;
    #[derive(Debug, Clone, Copy)]
    pub struct Admin;
    #[derive(Debug, Clone, Copy)]
    pub struct Web;
    #[derive(Debug, Clone, Copy)]
    pub struct Plugins;
    #[derive(Debug, Clone, Copy)]
    pub struct ContentGroup;

    impl CommandParent for Core {
        const PATH: &'static [&'static str] = &["core"];
    }
    impl CommandParent for Infra {
        const PATH: &'static [&'static str] = &["infra"];
    }
    impl CommandParent for Admin {
        const PATH: &'static [&'static str] = &["admin"];
    }
    impl CommandParent for Web {
        const PATH: &'static [&'static str] = &["web"];
    }
    impl CommandParent for Plugins {
        const PATH: &'static [&'static str] = &["plugins"];
    }
    impl CommandParent for ContentGroup {
        const PATH: &'static [&'static str] = &["core", "content"];
    }
}

/// A clap subcommand group a member contributes.
pub trait CliCommandGroup: Subcommand + DescribeCommand + Send + Sized + 'static {
    const NAME: &'static str;
    const ABOUT: &'static str = "";
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send;
}

/// A single clap command with its own arguments a member contributes.
pub trait CliCommandLeaf: Args + DescribeCommand + Send + Sized + 'static {
    const NAME: &'static str;
    const ABOUT: &'static str = "";
    const DISPLAY_ORDER: usize = REGISTERED_DISPLAY_ORDER;

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send;
}

/// A forwarded command line as the host classifies it: the subcommand path
/// it selects, the descriptor the runner would bootstrap it with, and the
/// member that owns it (`None` for a kernel command).
#[derive(Debug, Clone)]
pub struct DescribedCommand {
    pub path: Vec<String>,
    pub descriptor: CommandDescriptor,
    pub owner: Option<ExtensionId>,
}

/// What a command may ask of the CLI kernel.
///
/// [`CommandHost::describe_command_line`] classifies an argv (without the
/// binary name) against the host's command tree without running it. It fails
/// closed: a line that selects no command, or a command whose owner the
/// profile disables, is an error, never a description.
///
/// [`CommandHost::profile_path`] is the file the active profile was loaded
/// from; a command that edits the profile writes against it.
///
/// [`CommandHost::env_profile`] is the `SYSTEMPROMPT_PROFILE` value the runner
/// resolved at start-up. [`CommandHost::boot_config`] boots the profile at a
/// path with the secrets sources the composition admitted and returns its
/// configuration; the host serves it only to a command holding
/// [`KernelGrant::BootProfile`](super::KernelGrant::BootProfile) and refuses
/// every other caller. A command never boots a profile with sources of its
/// own.
///
/// [`CommandHost::admitted_views`] reads only the disabled-member list of the
/// profile at a path, admits the binary's composition against it and returns
/// the open [`AdmittedViews`]; nothing else in that profile is interpolated,
/// validated or returned, and no member code runs. The host serves it only to
/// a command holding
/// [`KernelGrant::TargetAdmission`](super::KernelGrant::TargetAdmission), at
/// most once per run.
///
/// [`CommandHost::profile`] is served only to a command holding
/// [`KernelGrant::BootProfile`](super::KernelGrant::BootProfile); every
/// other member command is refused it. A member command reaches kernel-held
/// material only through a grant listed on its reserved CLI path's row:
/// [`CommandHost::services_tree`] needs
/// [`KernelGrant::ServicesTree`](super::KernelGrant::ServicesTree) and reads
/// the boot context without a database; [`CommandHost::caller_session`] needs
/// [`KernelGrant::CallerSession`](super::KernelGrant::CallerSession) and
/// resolves the caller's own session; [`CommandHost::export_dir`] needs
/// [`KernelGrant::LocalExport`](super::KernelGrant::LocalExport) and is the
/// profile's local exports directory; [`CommandHost::use_session_context`]
/// needs [`KernelGrant::SessionContext`](super::KernelGrant::SessionContext)
/// and switches the caller's stored CLI session to a context the kernel first
/// confirms the caller owns; [`CommandHost::signing_keys`] needs
/// [`KernelGrant::SigningKeys`](super::KernelGrant::SigningKeys) and returns
/// the kernel-held [`DeploymentSigningKeys`] handle;
/// [`CommandHost::require_writable_plane`] needs
/// [`KernelGrant::ConfigPlaneWriter`](super::KernelGrant::ConfigPlaneWriter)
/// for that plane and refuses when the profile makes the plane file-managed.
/// [`CommandHost::identity_settings`] is served only to a command whose owner
/// is `identity-http` with core standing.
/// A command without the grant is refused,
/// and a command whose path lists no grant holds none.
///
/// [`CommandHost::render`] renders an intermediate output through the
/// kernel's sink (a command that redraws until interrupted); the output a
/// command returns is rendered once at exit, and a command that fails with
/// output attaches it with [`CommandError::with_output`] instead of rendering
/// it first. A command never writes its own renderer.
pub trait CommandHost: Send + Sync {
    fn output(&self) -> &CliConfig;
    fn prompter(&self) -> &dyn Prompter;
    fn is_database_scoped(&self) -> bool;
    fn profile(&self) -> Result<&Profile, CommandError>;
    fn profile_path(&self) -> Result<&Path, CommandError>;
    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>>;
    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>>;
    fn children(&self) -> &dyn ChildSupervisor;
    fn render(&self, output: &CommandOutput);
    fn describe_command_line(&self, command: &[String]) -> Result<DescribedCommand, CommandError>;
    fn env_profile(&self) -> Option<&str>;
    fn boot_config<'a>(
        &'a self,
        profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>>;
    fn admitted_views(&self, profile: &Path) -> Result<AdmittedViews, CommandError>;
    fn services_tree(&self) -> Result<ServicesTree, CommandError>;
    fn export_dir(&self) -> Result<PathBuf, CommandError>;
    fn caller_session(&self) -> BoxFuture<'_, Result<CallerSession, CommandError>>;
    fn use_session_context(&self, context: ContextId) -> BoxFuture<'_, Result<(), CommandError>>;
    fn signing_keys(&self) -> Result<Arc<dyn DeploymentSigningKeys>, CommandError>;
    fn require_writable_plane(
        &self,
        plane: GrantedPlane,
    ) -> BoxFuture<'_, Result<(), CommandError>>;
    fn identity_settings(&self) -> Result<IdentityCliSettings, CommandError>;
}
