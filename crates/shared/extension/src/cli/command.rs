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
//! (`web`) and `70` (`plugins`); `40` and `50` are the `cloud` and `analytics`
//! slots. A declaration that names no slot gets [`REGISTERED_DISPLAY_ORDER`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use clap::{Args, Subcommand};
use systemprompt_manifest::Profile;
use systemprompt_traits::DatabaseHandle;

use super::child::ChildSupervisor;
use super::descriptor::{DbUrlRouting, DescribeCommand};
use super::output::CommandOutput;
use super::prompt::Prompter;
use super::settings::CliConfig;
use crate::contribute::Resolved;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type CommandFuture<'a> = BoxFuture<'a, Result<CommandOutput, CommandError>>;

pub const REGISTERED_DISPLAY_ORDER: usize = 1000;

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct CommandError(#[from] Box<dyn std::error::Error + Send + Sync>);

impl CommandError {
    pub fn new(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self(error.into())
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

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

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

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Unsupported
    }

    fn run(
        self,
        host: &dyn CommandHost,
    ) -> impl Future<Output = Result<CommandOutput, CommandError>> + Send;
}

/// What a command may ask of the CLI kernel.
///
/// [`CommandHost::render`] renders an intermediate output through the
/// kernel's sink (a command that redraws until interrupted); the output a
/// command returns is rendered once at exit. A command never writes its own
/// renderer.
pub trait CommandHost: Send + Sync {
    fn output(&self) -> &CliConfig;
    fn prompter(&self) -> &dyn Prompter;
    fn is_database_scoped(&self) -> bool;
    fn profile(&self) -> Result<&Profile, CommandError>;
    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>>;
    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>>;
    fn children(&self) -> &dyn ChildSupervisor;
    fn render(&self, output: &CommandOutput);
}
