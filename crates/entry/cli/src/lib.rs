//! systemprompt.io command-line application.
//!
//! Implements the `systemprompt` binary: the command tree ([`admin`],
//! [`analytics`], [`build`], [`cloud`], [`core`], [`infrastructure`],
//! [`plugins`], [`web`]), output formatting, interactive prompts, and session
//! handling. [`run`] is the process entry point; [`CliConfig`] and the
//! settings enums control verbosity, color, and output format.

pub mod cli_settings;
mod commands;
pub mod descriptor;
pub mod environment;
pub mod interactive;
pub mod paths;
pub mod presentation;
mod runner;
pub mod session;
pub mod shared;

pub use cli_settings::{CliConfig, ColorMode, OutputFormat, VerbosityLevel};
pub use commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web};
pub use runner::run;
