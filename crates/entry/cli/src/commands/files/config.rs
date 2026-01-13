use anyhow::Result;
use clap::Args;
use systemprompt_core_files::{FilePersistenceMode, FilesConfig};

use super::types::{AllowedTypesOutput, FileConfigOutput, StoragePathsOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {}

pub fn execute(_args: ConfigArgs, _config: &CliConfig) -> Result<CommandResult<FileConfigOutput>> {
    let files_config = FilesConfig::get()?;
    let upload_config = files_config.upload();

    let persistence_mode = match upload_config.persistence_mode {
        FilePersistenceMode::ContextScoped => "context_scoped",
        FilePersistenceMode::UserLibrary => "user_library",
        FilePersistenceMode::Disabled => "disabled",
    };

    let allowed_types = AllowedTypesOutput {
        images: upload_config.allowed_types.images,
        documents: upload_config.allowed_types.documents,
        audio: upload_config.allowed_types.audio,
        video: upload_config.allowed_types.video,
    };

    let storage_paths = StoragePathsOutput {
        uploads: files_config.uploads().display().to_string(),
        images: files_config.images().display().to_string(),
        documents: files_config.documents().display().to_string(),
        audio: files_config.audio().display().to_string(),
        video: files_config.video().display().to_string(),
    };

    let output = FileConfigOutput {
        uploads_enabled: upload_config.enabled,
        max_file_size_bytes: upload_config.max_file_size_bytes,
        persistence_mode: persistence_mode.to_string(),
        storage_root: files_config.storage().display().to_string(),
        url_prefix: files_config.url_prefix().to_string(),
        allowed_types,
        storage_paths,
    };

    Ok(CommandResult::card(output).with_title("File Upload Configuration"))
}
