//! Interactive prompting helpers shared across commands.
//!
//! `Prompter` (in `systemprompt_extension::cli`) is the single seam over
//! operator interaction: production code uses [`DialoguerPrompter`] (terminal
//! prompts via `dialoguer`), tests use [`ScriptedPrompter`] with queued
//! answers. The flag-bridging helpers ([`resolve_required`], …) keep their
//! behaviour: interactive mode prompts, non-interactive mode falls back to a
//! default or fails with a "flag required" error.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};
use systemprompt_extension::cli::{CliConfig, CommandError, Prompter};


#[derive(Debug, Clone, Copy, Default)]
pub struct DialoguerPrompter;

impl DialoguerPrompter {
    fn confirm_answer(&self, message: &str, default: bool) -> Result<bool> {
        Ok(Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default)
            .interact()?)
    }

    fn input_answer(&self, prompt: &str) -> Result<String> {
        Ok(Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact_text()?)
    }

    fn input_with_default_answer(&self, prompt: &str, default: &str) -> Result<String> {
        Ok(Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default.to_owned())
            .interact_text()?)
    }

    fn select_answer(&self, prompt: &str, items: &[String]) -> Result<usize> {
        Ok(Select::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(items)
            .default(0)
            .interact()?)
    }

    fn password_answer(&self, prompt: &str) -> Result<String> {
        Ok(Password::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact()?)
    }
}

impl Prompter for DialoguerPrompter {
    fn confirm(&self, message: &str, default: bool) -> Result<bool, CommandError> {
        self.confirm_answer(message, default)
            .map_err(CommandError::new)
    }

    fn input(&self, prompt: &str) -> Result<String, CommandError> {
        self.input_answer(prompt)
            .map_err(CommandError::new)
    }

    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String, CommandError> {
        self.input_with_default_answer(prompt, default)
            .map_err(CommandError::new)
    }

    fn select(&self, prompt: &str, items: &[String]) -> Result<usize, CommandError> {
        self.select_answer(prompt, items)
            .map_err(CommandError::new)
    }

    fn password(&self, prompt: &str) -> Result<String, CommandError> {
        self.password_answer(prompt)
            .map_err(CommandError::new)
    }
}

#[derive(Debug, Default)]
pub struct ScriptedPrompter {
    answers: Mutex<VecDeque<String>>,
}

impl ScriptedPrompter {
    #[must_use]
    pub fn new(answers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            answers: Mutex::new(answers.into_iter().map(Into::into).collect()),
        }
    }

    fn next_answer(&self, prompt: &str) -> Result<String> {
        self.answers
            .lock()
            .map_err(|e| anyhow!("Scripted prompter lock poisoned: {e}"))?
            .pop_front()
            .ok_or_else(|| anyhow!("Scripted prompter exhausted at prompt: {prompt}"))
    }
}

impl ScriptedPrompter {
    fn confirm_answer(&self, message: &str, _default: bool) -> Result<bool> {
        let answer = self.next_answer(message)?;
        Ok(matches!(
            answer.to_lowercase().as_str(),
            "y" | "yes" | "true"
        ))
    }

    fn input_answer(&self, prompt: &str) -> Result<String> {
        self.next_answer(prompt)
    }

    fn input_with_default_answer(&self, prompt: &str, default: &str) -> Result<String> {
        match self.next_answer(prompt) {
            Ok(answer) if answer.is_empty() => Ok(default.to_owned()),
            other => other,
        }
    }

    fn select_answer(&self, prompt: &str, items: &[String]) -> Result<usize> {
        let answer = self.next_answer(prompt)?;
        let idx: usize = answer
            .parse()
            .map_err(|e| anyhow!("Scripted select answer '{answer}' is not an index: {e}"))?;
        if idx >= items.len() {
            return Err(anyhow!(
                "Scripted select index {idx} out of range for {} items",
                items.len()
            ));
        }
        Ok(idx)
    }

    fn password_answer(&self, prompt: &str) -> Result<String> {
        self.next_answer(prompt)
    }
}

impl Prompter for ScriptedPrompter {
    fn confirm(&self, message: &str, default: bool) -> Result<bool, CommandError> {
        self.confirm_answer(message, default)
            .map_err(CommandError::new)
    }

    fn input(&self, prompt: &str) -> Result<String, CommandError> {
        self.input_answer(prompt)
            .map_err(CommandError::new)
    }

    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String, CommandError> {
        self.input_with_default_answer(prompt, default)
            .map_err(CommandError::new)
    }

    fn select(&self, prompt: &str, items: &[String]) -> Result<usize, CommandError> {
        self.select_answer(prompt, items)
            .map_err(CommandError::new)
    }

    fn password(&self, prompt: &str) -> Result<String, CommandError> {
        self.password_answer(prompt)
            .map_err(CommandError::new)
    }
}

pub fn resolve_required<T, F>(
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
        None => Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        )),
    }
}

pub fn select_from_list<T: ToString + Clone>(
    prompter: &dyn Prompter,
    prompt: &str,
    items: &[T],
    flag_name: &str,
    config: &CliConfig,
) -> Result<T> {
    if items.is_empty() {
        return Err(anyhow!("No items available for selection"));
    }

    if !config.is_interactive() {
        return Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        ));
    }

    let display: Vec<String> = items.iter().map(ToString::to_string).collect();
    let idx = prompter.select(prompt, &display)?;
    Ok(items[idx].clone())
}

pub fn select_index(
    prompter: &dyn Prompter,
    prompt: &str,
    items: &[&str],
    config: &CliConfig,
) -> Result<Option<usize>> {
    if !config.is_interactive() {
        return Ok(None);
    }

    let display: Vec<String> = items.iter().map(|s| (*s).to_owned()).collect();
    Ok(Some(prompter.select(prompt, &display)?))
}

pub fn prompt_input(
    prompter: &dyn Prompter,
    prompt: &str,
    flag_name: &str,
    config: &CliConfig,
) -> Result<String> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        ));
    }

    prompter.input(prompt)
}

pub fn prompt_input_with_default(
    prompter: &dyn Prompter,
    prompt: &str,
    default: &str,
    config: &CliConfig,
) -> Result<String> {
    if !config.is_interactive() {
        return Ok(default.to_owned());
    }

    prompter.input_with_default(prompt, default)
}

pub fn confirm_optional(
    prompter: &dyn Prompter,
    message: &str,
    default: bool,
    config: &CliConfig,
) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(default);
    }

    prompter.confirm(message, default)
}
