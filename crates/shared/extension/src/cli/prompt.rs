//! The operator-interaction seam a command prompts through.
//!
//! The CLI kernel supplies the terminal implementation and tests supply a
//! scripted one; a command reaches its [`Prompter`] through its
//! `CommandHost`. [`require_confirmation`] never prints: in a non-interactive
//! run it refuses unless confirmation was skipped, and a declined prompt is a
//! typed refusal. [`resolve_required`] takes a flag's value, prompts for it in
//! an interactive run, and otherwise refuses with [`FlagRequired`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::command::CommandError;
use super::settings::CliConfig;

/// Prompts the operator; implemented by the CLI kernel.
pub trait Prompter: Send + Sync {
    fn confirm(&self, message: &str, default: bool) -> Result<bool, CommandError>;
    fn input(&self, prompt: &str) -> Result<String, CommandError>;
    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String, CommandError>;
    fn select(&self, prompt: &str, items: &[String]) -> Result<usize, CommandError>;
    fn password(&self, prompt: &str) -> Result<String, CommandError>;
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ConfirmationRefused {
    #[error("--yes is required in non-interactive mode")]
    NonInteractive,
    #[error("Operation cancelled")]
    Declined,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("--{flag} is required in non-interactive mode")]
pub struct FlagRequired {
    pub flag: &'static str,
}

pub fn resolve_required<T, E, F>(
    value: Option<T>,
    flag: &'static str,
    config: &CliConfig,
    prompt: F,
) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
    E: From<FlagRequired>,
{
    match value {
        Some(value) => Ok(value),
        None if config.is_interactive() => prompt(),
        None => Err(FlagRequired { flag }.into()),
    }
}

pub fn require_confirmation(
    prompter: &dyn Prompter,
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<(), CommandError> {
    require_confirmation_with(prompter, message, skip_confirmation, false, config)
}

pub fn require_confirmation_default_yes(
    prompter: &dyn Prompter,
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<(), CommandError> {
    require_confirmation_with(prompter, message, skip_confirmation, true, config)
}

fn require_confirmation_with(
    prompter: &dyn Prompter,
    message: &str,
    skip_confirmation: bool,
    default: bool,
    config: &CliConfig,
) -> Result<(), CommandError> {
    if skip_confirmation {
        return Ok(());
    }
    if !config.is_interactive() {
        return Err(CommandError::new(ConfirmationRefused::NonInteractive));
    }
    if prompter.confirm(message, default)? {
        Ok(())
    } else {
        Err(CommandError::new(ConfirmationRefused::Declined))
    }
}
