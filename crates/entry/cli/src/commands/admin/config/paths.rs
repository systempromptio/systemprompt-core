//! `admin config paths` command: show and validate the configured filesystem
//! paths.
//!
//! [`PathsCommands`] reports the system, services, bin, web, storage, and
//! `GeoIP` paths from the active profile and checks whether each required path
//! exists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Subcommand;
use std::path::Path;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::types::{PathInfo, PathValidation, PathsConfigOutput, PathsValidateOutput};
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum PathsCommands {
    #[command(about = "Show paths configuration", alias = "list")]
    Show,

    #[command(about = "Validate that all configured paths exist")]
    Validate,
}

pub fn execute(command: PathsCommands, config: &CliConfig) -> Result<()> {
    match command {
        PathsCommands::Show => execute_show(config),
        PathsCommands::Validate => execute_validate(config),
    }
}

pub(super) fn execute_show(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let output = PathsConfigOutput {
        system: PathInfo {
            path: profile.paths.system.clone(),
            exists: Path::new(&profile.paths.system).exists(),
        },
        services: PathInfo {
            path: profile.paths.services.clone(),
            exists: Path::new(&profile.paths.services).exists(),
        },
        bin: PathInfo {
            path: profile.paths.bin.clone(),
            exists: Path::new(&profile.paths.bin).exists(),
        },
        web_path: profile.paths.web_path.as_ref().map(|p| PathInfo {
            path: p.clone(),
            exists: Path::new(p).exists(),
        }),
        storage: profile.paths.storage.as_ref().map(|p| PathInfo {
            path: p.clone(),
            exists: Path::new(p).exists(),
        }),
        geoip_database: profile.paths.geoip_database.as_ref().map(|p| PathInfo {
            path: p.clone(),
            exists: Path::new(p).exists(),
        }),
    };

    render_result(
        &CommandOutput::card_value("Paths Configuration", &output),
        config,
    );

    Ok(())
}

pub(super) fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let validations = collect_path_validations(profile);
    let valid = validations.iter().filter(|v| v.required).all(|v| v.exists);

    let output = PathsValidateOutput {
        valid,
        paths: validations,
    };

    render_result(
        &CommandOutput::table_of(vec!["name", "path", "exists", "required"], &output.paths)
            .with_title("Paths Validation"),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        if valid {
            CliService::success("All required paths exist");
        } else {
            CliService::error("Some required paths are missing");
        }
    }

    Ok(())
}

fn collect_path_validations(profile: &Profile) -> Vec<PathValidation> {
    let mut validations = vec![
        path_validation("system", &profile.paths.system, true),
        path_validation("services", &profile.paths.services, true),
        path_validation("bin", &profile.paths.bin, true),
    ];

    if let Some(web_path) = &profile.paths.web_path {
        validations.push(path_validation("web_path", web_path, false));
    }

    if let Some(storage) = &profile.paths.storage {
        validations.push(path_validation("storage", storage, false));
    }

    if let Some(geoip) = &profile.paths.geoip_database {
        validations.push(path_validation("geoip_database", geoip, false));
    }

    validations.push(path_validation("config", &profile.paths.config(), false));
    validations.push(path_validation(
        "ai_config",
        &profile.paths.ai_config(),
        false,
    ));
    validations.push(path_validation(
        "content_config",
        &profile.paths.content_config(),
        false,
    ));

    validations
}

fn path_validation(name: &str, path: &str, required: bool) -> PathValidation {
    PathValidation {
        name: name.to_owned(),
        path: path.to_owned(),
        exists: Path::new(path).exists(),
        required,
    }
}
