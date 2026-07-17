//! systemprompt.io command-line application.
//!
//! Implements the `systemprompt` binary: the command tree ([`admin`],
//! [`analytics`], [`build`], [`cloud`], [`core`], [`infrastructure`],
//! [`plugins`], [`web`]), output formatting, interactive prompts, and session
//! handling. [`run`] is the process entry point; [`CliConfig`] and the
//! settings enums control verbosity, color, and output format.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cli_settings;
mod commands;
pub mod context;
pub mod descriptor;
pub mod env_overrides;
pub mod environment;
pub mod interactive;
pub mod paths;
pub mod presentation;
mod runner;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};
pub use context::CommandContext;
pub use env_overrides::{EnvOverrides, SessionEnv};
pub use interactive::{DialoguerPrompter, Prompter, ScriptedPrompter};
pub use runner::run;
