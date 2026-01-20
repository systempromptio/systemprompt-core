use anyhow::{Context, Result};
use clap::Args;
use std::process::Command;

use super::types::ConfigSection;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct EditArgs {
    #[arg(value_name = "SECTION")]
    pub section: String,
}

pub fn execute(args: EditArgs, _config: &CliConfig) -> Result<()> {
    let section = args.section.parse::<ConfigSection>()?;
    let file_path = section.file_path()?;

    if !file_path.exists() {
        anyhow::bail!(
            "Config file not found: {}\nSection '{}' may not be configured.",
            file_path.display(),
            section
        );
    }

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    println!("Opening {} in {}...", file_path.display(), editor);

    let status = Command::new(&editor)
        .arg(&file_path)
        .status()
        .with_context(|| format!("Failed to run editor: {}", editor))?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    println!("Config file saved: {}", file_path.display());

    Ok(())
}
