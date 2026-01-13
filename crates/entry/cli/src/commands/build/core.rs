use anyhow::{bail, Context, Result};
use clap::Args;
use std::process::Command;
use std::time::Instant;

use systemprompt_core_logging::CliService;

use super::types::CoreBuildOutput;
use crate::shared::command_result::CommandResult;
use crate::shared::project::ProjectRoot;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct CoreArgs {
    #[arg(long, short = 'r', help = "Build in release mode (production)")]
    pub release: bool,

    #[arg(long, help = "Build with SQLX_OFFLINE=true (no database required)")]
    pub offline: bool,
}

pub fn execute(args: CoreArgs, config: &CliConfig) -> Result<CommandResult<CoreBuildOutput>> {
    let project_root = ProjectRoot::discover()?;
    let root = project_root.as_path();

    let mode = if args.release { "release" } else { "debug" };

    if !config.is_json_output() {
        CliService::section(&format!("Building Core ({})", mode));
    }

    let start = Instant::now();

    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--workspace").current_dir(root);

    if args.release {
        cmd.arg("--release");
    }

    if args.offline {
        cmd.env("SQLX_OFFLINE", "true");
    }

    let status = cmd.status().context("Failed to execute cargo build")?;

    let duration = start.elapsed().as_secs_f64();

    if !status.success() {
        if !config.is_json_output() {
            CliService::error(&format!("Core build failed after {:.1}s", duration));
        }
        bail!("Cargo build failed");
    }

    let output = CoreBuildOutput {
        target: "workspace".to_string(),
        mode: mode.to_string(),
        status: "success".to_string(),
        duration_secs: Some(duration),
    };

    if !config.is_json_output() {
        CliService::success(&format!("Core built successfully in {:.1}s", duration));
    }

    Ok(CommandResult::card(output).with_title("Core Build"))
}
