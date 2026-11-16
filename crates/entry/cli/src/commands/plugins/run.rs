//! `plugins run` command locating and spawning a CLI extension binary.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use crate::kernel::KernelHost;
use clap::Args;
use std::process::{Command, Stdio};
use systemprompt_loader::ExtensionLoader;

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(help = "Extension name (binary name or manifest name)")]
    pub extension: String,

    #[arg(help = "Arguments to pass to the extension", trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub(super) async fn execute(args: RunArgs, host: &dyn KernelHost) -> Result<()> {
    let project_root = std::env::current_dir().context("Failed to get current directory")?;

    let extension = ExtensionLoader::find_cli_extension(&project_root, &args.extension)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "CLI extension '{}' not found. Use 'plugins list --type cli' to see available CLI \
                 extensions",
                args.extension
            )
        })?;

    let binary_name = extension
        .binary_name()
        .ok_or_else(|| anyhow::anyhow!("Extension '{}' has no binary defined", args.extension))?;

    let binary_path =
        ExtensionLoader::get_cli_binary_path(&project_root, binary_name).ok_or_else(|| {
            anyhow::anyhow!(
                "Binary '{}' not found. Build with: cargo build --release --package {}",
                binary_name,
                binary_name
            )
        })?;

    let mut cmd = Command::new(&binary_path);
    cmd.args(&args.args)
        .envs(host.child_env()?)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if host.output().is_json_output() && extension.supports_json_output() {
        cmd.arg("--json");
    }

    let children = host.children().handle();
    let status = tokio::task::spawn_blocking(move || children.status(cmd))
        .await
        .context("Extension supervisor task failed")?
        .with_context(|| {
            format!(
                "Failed to execute extension binary: {}",
                binary_path.display()
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(1);
        anyhow::bail!("Extension exited with code {}", code)
    }
}
