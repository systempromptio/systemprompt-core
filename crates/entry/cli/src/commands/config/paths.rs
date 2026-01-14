use anyhow::Result;
use clap::Subcommand;
use std::path::Path;
use systemprompt_core_logging::CliService;
use systemprompt_models::ProfileBootstrap;

use super::types::{PathInfo, PathValidation, PathsConfigOutput, PathsValidateOutput};
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Subcommand)]
pub enum PathsCommands {
    #[command(about = "Show paths configuration")]
    Show,

    #[command(about = "Validate that all configured paths exist")]
    Validate,
}

pub fn execute(command: PathsCommands, config: &CliConfig) -> Result<()> {
    match command {
        PathsCommands::Show => execute_show(),
        PathsCommands::Validate => execute_validate(config),
    }
}

fn execute_show() -> Result<()> {
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

    render_result(&CommandResult::card(output).with_title("Paths Configuration"));

    Ok(())
}

fn execute_validate(config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;

    let mut validations: Vec<PathValidation> = Vec::new();

    // Required paths
    validations.push(PathValidation {
        name: "system".to_string(),
        path: profile.paths.system.clone(),
        exists: Path::new(&profile.paths.system).exists(),
        required: true,
    });

    validations.push(PathValidation {
        name: "services".to_string(),
        path: profile.paths.services.clone(),
        exists: Path::new(&profile.paths.services).exists(),
        required: true,
    });

    validations.push(PathValidation {
        name: "bin".to_string(),
        path: profile.paths.bin.clone(),
        exists: Path::new(&profile.paths.bin).exists(),
        required: true,
    });

    // Optional paths
    if let Some(web_path) = &profile.paths.web_path {
        validations.push(PathValidation {
            name: "web_path".to_string(),
            path: web_path.clone(),
            exists: Path::new(web_path).exists(),
            required: false,
        });
    }

    if let Some(storage) = &profile.paths.storage {
        validations.push(PathValidation {
            name: "storage".to_string(),
            path: storage.clone(),
            exists: Path::new(storage).exists(),
            required: false,
        });
    }

    if let Some(geoip) = &profile.paths.geoip_database {
        validations.push(PathValidation {
            name: "geoip_database".to_string(),
            path: geoip.clone(),
            exists: Path::new(geoip).exists(),
            required: false,
        });
    }

    // Derived paths
    let config_path = profile.paths.config();
    validations.push(PathValidation {
        name: "config".to_string(),
        path: config_path.clone(),
        exists: Path::new(&config_path).exists(),
        required: false,
    });

    let ai_config_path = profile.paths.ai_config();
    validations.push(PathValidation {
        name: "ai_config".to_string(),
        path: ai_config_path.clone(),
        exists: Path::new(&ai_config_path).exists(),
        required: false,
    });

    let content_config_path = profile.paths.content_config();
    validations.push(PathValidation {
        name: "content_config".to_string(),
        path: content_config_path.clone(),
        exists: Path::new(&content_config_path).exists(),
        required: false,
    });

    // Calculate validity - only required paths matter
    let valid = validations
        .iter()
        .filter(|v| v.required)
        .all(|v| v.exists);

    let output = PathsValidateOutput {
        valid,
        paths: validations,
    };

    render_result(&CommandResult::table(output.clone()).with_title("Paths Validation"));

    if config.output_format() == OutputFormat::Table {
        if valid {
            CliService::success("All required paths exist");
        } else {
            CliService::error("Some required paths are missing");
        }
    }

    Ok(())
}
