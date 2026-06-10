//! Interactive prompting helpers shared across commands.
//!
//! [`Prompter`] is the single seam over operator interaction: production code
//! uses [`DialoguerPrompter`] (terminal prompts via `dialoguer`), tests use
//! [`ScriptedPrompter`] with queued answers. The flag-bridging helpers
//! ([`require_confirmation`], [`resolve_required`], …) keep their behaviour:
//! interactive mode prompts, non-interactive mode falls back to a default or
//! fails with a "flag required" error.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::CliConfig;
use anyhow::{Result, anyhow};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, Password, Select};

pub trait Prompter: Send + Sync {
    fn confirm(&self, message: &str, default: bool) -> Result<bool>;
    fn input(&self, prompt: &str) -> Result<String>;
    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String>;
    fn select(&self, prompt: &str, items: &[String]) -> Result<usize>;
    fn password(&self, prompt: &str) -> Result<String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DialoguerPrompter;

impl Prompter for DialoguerPrompter {
    fn confirm(&self, message: &str, default: bool) -> Result<bool> {
        Ok(Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default)
            .interact()?)
    }

    fn input(&self, prompt: &str) -> Result<String> {
        Ok(Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact_text()?)
    }

    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String> {
        Ok(Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(default.to_owned())
            .interact_text()?)
    }

    fn select(&self, prompt: &str, items: &[String]) -> Result<usize> {
        Ok(Select::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .items(items)
            .default(0)
            .interact()?)
    }

    fn password(&self, prompt: &str) -> Result<String> {
        Ok(Password::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .interact()?)
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

impl Prompter for ScriptedPrompter {
    fn confirm(&self, message: &str, _default: bool) -> Result<bool> {
        let answer = self.next_answer(message)?;
        Ok(matches!(
            answer.to_lowercase().as_str(),
            "y" | "yes" | "true"
        ))
    }

    fn input(&self, prompt: &str) -> Result<String> {
        self.next_answer(prompt)
    }

    fn input_with_default(&self, prompt: &str, default: &str) -> Result<String> {
        match self.next_answer(prompt) {
            Ok(answer) if answer.is_empty() => Ok(default.to_owned()),
            other => other,
        }
    }

    fn select(&self, prompt: &str, items: &[String]) -> Result<usize> {
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

    fn password(&self, prompt: &str) -> Result<String> {
        self.next_answer(prompt)
    }
}

pub fn require_confirmation(
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<()> {
    require_confirmation_with(
        &DialoguerPrompter,
        message,
        skip_confirmation,
        false,
        config,
    )
}

pub fn require_confirmation_default_yes(
    message: &str,
    skip_confirmation: bool,
    config: &CliConfig,
) -> Result<()> {
    require_confirmation_with(&DialoguerPrompter, message, skip_confirmation, true, config)
}

pub fn require_confirmation_with(
    prompter: &dyn Prompter,
    message: &str,
    skip_confirmation: bool,
    default: bool,
    config: &CliConfig,
) -> Result<()> {
    if skip_confirmation {
        return Ok(());
    }

    if !config.is_interactive() {
        return Err(anyhow!("--yes is required in non-interactive mode"));
    }

    if prompter.confirm(message, default)? {
        Ok(())
    } else {
        Err(anyhow!("Operation cancelled"))
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
    let idx = DialoguerPrompter.select(prompt, &display)?;
    Ok(items[idx].clone())
}

pub fn select_index(prompt: &str, items: &[&str], config: &CliConfig) -> Result<Option<usize>> {
    if !config.is_interactive() {
        return Ok(None);
    }

    let display: Vec<String> = items.iter().map(|s| (*s).to_owned()).collect();
    Ok(Some(DialoguerPrompter.select(prompt, &display)?))
}

pub fn prompt_input(prompt: &str, flag_name: &str, config: &CliConfig) -> Result<String> {
    if !config.is_interactive() {
        return Err(anyhow!(
            "--{} is required in non-interactive mode",
            flag_name
        ));
    }

    DialoguerPrompter.input(prompt)
}

pub fn prompt_input_with_default(
    prompt: &str,
    default: &str,
    config: &CliConfig,
) -> Result<String> {
    if !config.is_interactive() {
        return Ok(default.to_owned());
    }

    DialoguerPrompter.input_with_default(prompt, default)
}

pub fn confirm_optional(message: &str, default: bool, config: &CliConfig) -> Result<bool> {
    if !config.is_interactive() {
        return Ok(default);
    }

    DialoguerPrompter.confirm(message, default)
}
