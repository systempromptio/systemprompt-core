//! systemprompt.io command-line application.
//!
//! The library behind the `systemprompt` binary (built by the `systemprompt`
//! facade crate, which links the capability crates that register commands):
//! the command tree ([`admin`], [`build`], [`core`], [`infrastructure`],
//! [`plugins`], [`web`], plus every group a linked crate registers), output
//! formatting, interactive prompts, and session handling. [`run_composed`] is
//! the whole of a composed binary's main function: the facade hands it the
//! declared commands and the admission closure; it builds the runtime and the
//! command registry, loads the profile, refuses a command whose owner the
//! profile disables ([`CliRunError::CommandDisabled`]), admits the
//! composition once, runs the command and maps an interrupted run to exit
//! status 130.
//!
//! [`kernel`] holds the command registry and the helpers the kernel's own
//! groups share.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod boot;
pub mod commands;
pub mod context;
pub mod env_overrides;
pub mod environment;
mod host;
pub mod interactive;
pub mod kernel;
pub mod paths;
pub mod presentation;
pub mod runner;
pub mod session;
pub mod shared;

pub use commands::{admin, build, core, infrastructure, plugins, web};
pub use context::CommandContext;
pub use env_overrides::{EnvOverrides, SessionEnv};
pub use interactive::{DialoguerPrompter, ScriptedPrompter};
pub use runner::{AdmitFn, CliRunError, args, run_composed};

#[doc(hidden)]
pub use inventory as __inventory;
