use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub fn run_command_in_dir(command: &str, args: &[&str], directory: &PathBuf) -> Result<()> {
    let status = Command::new(command)
        .args(args)
        .current_dir(directory)
        .status()
        .with_context(|| format!("Failed to run: {} {}", command, args.join(" ")))?;

    if !status.success() {
        return Err(anyhow!("Command failed: {} {}", command, args.join(" ")));
    }

    Ok(())
}
