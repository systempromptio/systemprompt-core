//! Interactive content-type selection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, anyhow};
use systemprompt_models::content_config::ContentConfigRaw;

use crate::interactive::Prompter;

pub fn prompt_content_type_selection(
    prompter: &dyn Prompter,
    config: &ContentConfigRaw,
    prompt: &str,
) -> Result<String> {
    let mut names: Vec<String> = config.content_sources.keys().cloned().collect();
    names.sort();

    if names.is_empty() {
        return Err(anyhow!("No content types configured"));
    }

    let selection = prompter.select(prompt, &names)?;
    Ok(names[selection].clone())
}
