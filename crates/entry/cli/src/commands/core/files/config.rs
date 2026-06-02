use anyhow::Result;
use clap::Args;
use systemprompt_files::{FilePersistenceMode, FilesConfig};

use super::types::{AllowedTypesOutput, FileConfigOutput, StoragePathsOutput};
use crate::CliConfig;
use crate::shared::CommandOutput;

#[derive(Debug, Clone, Copy, Args)]
pub struct ConfigArgs;

pub(super) fn execute(_args: ConfigArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let files_config = FilesConfig::get()?;
    let upload_config = files_config.upload();

    let persistence_mode = match upload_config.persistence_mode {
        FilePersistenceMode::ContextScoped => "context_scoped",
        FilePersistenceMode::UserLibrary => "user_library",
        FilePersistenceMode::Disabled => "disabled",
    };

    let mut allowed = Vec::new();
    if upload_config.allowed_types.images {
        allowed.push("images".to_owned());
    }
    if upload_config.allowed_types.documents {
        allowed.push("documents".to_owned());
    }
    if upload_config.allowed_types.audio {
        allowed.push("audio".to_owned());
    }
    if upload_config.allowed_types.video {
        allowed.push("video".to_owned());
    }
    let allowed_types = AllowedTypesOutput { allowed };

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
        persistence_mode: persistence_mode.to_owned(),
        storage_root: files_config.storage().display().to_string(),
        url_prefix: files_config.url_prefix().to_owned(),
        allowed_types,
        storage_paths,
    };

    Ok(CommandOutput::card_value(
        "File Upload Configuration",
        &output,
    ))
}
