use anyhow::{bail, Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use systemprompt_core_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use super::types::WebBuildOutput;
use crate::shared::command_result::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Copy, Args)]
pub struct WebArgs {
    #[arg(long, short = 'r', help = "Build in production mode")]
    pub release: bool,
}

pub fn execute(args: WebArgs, config: &CliConfig) -> Result<CommandResult<WebBuildOutput>> {
    let profile = ProfileBootstrap::get().context(
        "Profile required for web build. Set SYSTEMPROMPT_PROFILE environment variable.",
    )?;

    let web_dir = PathBuf::from(profile.paths.web_path_resolved());
    let web_config = profile.paths.web_config();
    let web_metadata = profile.paths.web_metadata();

    if !web_dir.exists() {
        bail!(
            "Web directory not found: {}\n\
             Configure paths.web_path in your profile, or ensure paths.system/web exists.",
            web_dir.display()
        );
    }

    let mode = if args.release { "production" } else { "development" };
    let build_script = if args.release { "build:prod" } else { "build" };

    if !config.is_json_output() {
        CliService::section(&format!("Building Web ({})", mode));
        CliService::key_value("Web directory", &web_dir.display().to_string());
        CliService::key_value("Config", &web_config);
        CliService::key_value("Metadata", &web_metadata);
    }

    let start = Instant::now();

    let status = Command::new("npm")
        .args(["run", build_script])
        .env("SYSTEMPROMPT_WEB_CONFIG_PATH", &web_config)
        .env("SYSTEMPROMPT_WEB_METADATA_PATH", &web_metadata)
        .current_dir(&web_dir)
        .status()
        .context("Failed to execute npm build")?;

    let duration = start.elapsed().as_secs_f64();
    let output_dir = web_dir.join("dist").display().to_string();

    if !status.success() {
        if !config.is_json_output() {
            CliService::error(&format!("Web build failed after {:.1}s", duration));
        }
        bail!("npm build failed");
    }

    let output = WebBuildOutput {
        target: "web".to_string(),
        mode: mode.to_string(),
        status: "success".to_string(),
        output_dir,
        duration_secs: Some(duration),
    };

    if !config.is_json_output() {
        CliService::success(&format!("Web built successfully in {:.1}s", duration));
        CliService::key_value("Output", &output.output_dir);
    }

    Ok(CommandResult::card(output).with_title("Web Build"))
}
