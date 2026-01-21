use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use systemprompt_logging::CliService;
use systemprompt_models::profile::RateLimitsConfig;
use systemprompt_models::ProfileBootstrap;

use super::helpers::{load_profile_for_edit, save_profile};
use super::{ExportArgs, ImportArgs};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{ExportOutput, ImportOutput};

pub fn execute_export(args: &ExportArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let limits = &profile.rate_limits;

    let content = match args.format.as_str() {
        "yaml" | "yml" => {
            serde_yaml::to_string(limits).context("Failed to serialize rate limits to YAML")?
        },
        "json" => serde_json::to_string_pretty(limits)
            .context("Failed to serialize rate limits to JSON")?,
        _ => bail!("Unknown format: {}. Valid formats: yaml, json", args.format),
    };

    fs::write(&args.output, &content)
        .with_context(|| format!("Failed to write to file: {}", args.output))?;

    let output = ExportOutput {
        path: args.output.clone(),
        format: args.format.clone(),
        message: format!("Exported rate limits to {}", args.output),
    };

    render_result(&CommandResult::text(output).with_title("Rate Limits Exported"));

    if config.output_format() == OutputFormat::Table {
        CliService::success(&format!("Exported to {}", args.output));
    }

    Ok(())
}

pub fn execute_import(args: &ImportArgs, config: &CliConfig) -> Result<()> {
    if !args.yes && !config.is_interactive() {
        bail!("--yes is required in non-interactive mode");
    }

    let path = Path::new(&args.file);
    if !path.exists() {
        bail!("File not found: {}", args.file);
    }

    let content = fs::read_to_string(&args.file)
        .with_context(|| format!("Failed to read file: {}", args.file))?;

    let is_json = Path::new(&args.file)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));

    let new_limits: RateLimitsConfig = if is_json {
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON from: {}", args.file))?
    } else {
        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse YAML from: {}", args.file))?
    };

    if !args.yes && config.is_interactive() {
        CliService::warning(&format!("This will import rate limits from {}", args.file));
        if !CliService::confirm("Proceed with import?")? {
            CliService::info("Import cancelled");
            return Ok(());
        }
    }

    let profile_path = ProfileBootstrap::get_path()?;
    let mut profile = load_profile_for_edit(profile_path)?;
    profile.rate_limits = new_limits;
    save_profile(&profile, profile_path)?;

    let output = ImportOutput {
        path: args.file.clone(),
        changes: vec![],
        message: format!("Imported rate limits from {}", args.file),
    };

    render_result(&CommandResult::text(output).with_title("Rate Limits Imported"));

    if config.output_format() == OutputFormat::Table {
        CliService::warning("Restart services for changes to take effect");
    }

    Ok(())
}
