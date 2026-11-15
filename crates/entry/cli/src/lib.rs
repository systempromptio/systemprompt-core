//! systemprompt.io command-line application.
//!
//! The library behind the `systemprompt` binary (built by the `systemprompt`
//! facade crate, which links the capability crates that register commands):
//! the command tree ([`admin`], [`build`], [`core`], [`infrastructure`],
//! [`plugins`], [`web`], plus every group a linked crate registers), output
//! formatting, interactive prompts, and session handling. [`run`] is the async
//! entry point; [`run_with`] is the same for an embedding binary that names
//! its own build, which the health and probe bodies report. [`run_process`]
//! and [`run_process_with`] are the whole of a binary's main function: they
//! build the runtime, run the command and map an interrupted run to exit
//! status 130. [`CliConfig`] and the settings enums control verbosity, color,
//! and output format.
//!
//! [`kernel`] is the surface command groups build on, and the seam through
//! which a group defined in another crate registers itself with
//! [`register_cli_command!`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod boot;
pub mod cli_settings;
pub mod commands;
pub mod context;
pub mod descriptor;
pub mod env_overrides;
pub mod environment;
pub mod interactive;
pub mod kernel;
pub mod paths;
pub mod presentation;
pub mod runner;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{admin, build, core, infrastructure, plugins, web};
pub use context::CommandContext;
pub use env_overrides::{EnvOverrides, SessionEnv};
pub use interactive::{DialoguerPrompter, Prompter, ScriptedPrompter};
pub use runner::{args, run, run_process, run_process_with, run_with};

#[doc(hidden)]
pub use inventory as __inventory;
