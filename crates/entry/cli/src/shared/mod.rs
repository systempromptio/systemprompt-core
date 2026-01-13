pub mod command_result;
pub mod docker;
pub mod paths;
pub mod process;
pub mod profile;
pub mod project;
pub mod web;

pub use command_result::{
    render_result, ArtifactType, ChartType, CommandResult, KeyValueItem, KeyValueOutput,
    RenderingHints, SuccessOutput, TableOutput, TextOutput,
};

use anyhow::{anyhow, Result};

use crate::CliConfig;

/// Resolve an optional input value, falling back to interactive prompt if available.
///
/// If the value is `Some`, returns it.
/// If the value is `None` and interactive mode is available, calls `prompt_fn`.
/// If the value is `None` and non-interactive mode, returns an error.
pub fn resolve_input<T, F>(
    value: Option<T>,
    flag_name: &str,
    config: &CliConfig,
    prompt_fn: F,
) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    match value {
        Some(v) => Ok(v),
        None if config.is_interactive() => prompt_fn(),
        None => Err(anyhow!("--{} is required in non-interactive mode", flag_name)),
    }
}
