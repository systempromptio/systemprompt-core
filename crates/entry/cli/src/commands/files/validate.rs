use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;
use systemprompt_core_files::{FileValidator, FilesConfig};
use tokio::fs;

use super::types::FileValidationOutput;
use super::upload::detect_mime_type;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ValidateArgs {
    #[arg(help = "Path to file to validate")]
    pub file_path: PathBuf,
}

pub fn execute(args: ValidateArgs, _config: &CliConfig) -> Result<CommandResult<FileValidationOutput>> {
    let file_path = args
        .file_path
        .canonicalize()
        .map_err(|e| anyhow!("File not found: {} - {}", args.file_path.display(), e))?;

    let metadata = std::fs::metadata(&file_path)?;
    let size_bytes = metadata.len();

    let mime_type = detect_mime_type(&file_path);

    let files_config = FilesConfig::get()?;
    let upload_config = files_config.upload();
    let validator = FileValidator::new(*upload_config);

    let (valid, category, errors) = match validator.validate(&mime_type, size_bytes) {
        Ok(cat) => (true, cat.display_name().to_string(), vec![]),
        Err(e) => (false, "unknown".to_string(), vec![e.to_string()]),
    };

    let output = FileValidationOutput {
        valid,
        mime_type,
        category,
        size_bytes,
        max_size_bytes: upload_config.max_file_size_bytes,
        errors,
    };

    let title = if valid {
        "File Valid"
    } else {
        "File Invalid"
    };

    Ok(CommandResult::card(output).with_title(title))
}
