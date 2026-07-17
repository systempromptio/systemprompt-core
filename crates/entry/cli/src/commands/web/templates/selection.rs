//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};

use crate::interactive::Prompter;

use super::super::types::TemplatesConfig;

pub fn prompt_template_selection(
    prompter: &dyn Prompter,
    config: &TemplatesConfig,
    prompt: &str,
) -> Result<String> {
    let mut names: Vec<String> = config.templates.keys().cloned().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No templates configured"));
    }

    let selection = prompter.select(prompt, &names)?;
    Ok(names[selection].clone())
}
