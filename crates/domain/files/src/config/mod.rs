//! Profile-driven configuration for the files crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod document_limits;
mod documents;
mod types;
mod validator;

pub use document_limits::DocumentLimits;
pub use documents::{DocumentsConfig, MAX_DOCUMENT_UPLOAD_BYTES, MIN_DOCUMENT_UPLOAD_BYTES};
pub use types::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};
pub use validator::FilesConfigValidator;

use std::path::{Path, PathBuf};
use systemprompt_cloud::constants::storage;
use systemprompt_config::paths::AppPaths;
use systemprompt_manifest::profile::Profile;

use crate::error::{FilesError, FilesResult};
use types::FilesConfigWrapper;

#[derive(Debug, Clone)]
pub struct FilesConfig {
    storage_root: PathBuf,
    url_prefix: String,
    cache_control: Option<String>,
    upload: FileUploadConfig,
    documents: DocumentsConfig,
}

impl FilesConfig {
    pub fn from_profile(profile: &Profile, paths: &AppPaths) -> FilesResult<Self> {
        let storage_root = profile
            .paths
            .storage
            .as_ref()
            .ok_or_else(|| FilesError::Config("paths.storage not configured in profile".into()))?
            .clone();

        let yaml_config = Self::load_yaml_config(paths)?;

        Ok(Self::new(PathBuf::from(storage_root), yaml_config))
    }

    #[must_use]
    pub fn new(storage_root: PathBuf, yaml: FilesConfigYaml) -> Self {
        Self {
            storage_root,
            url_prefix: yaml.url_prefix,
            cache_control: yaml.cache_control,
            upload: yaml.upload,
            documents: yaml.documents,
        }
    }

    pub(super) fn load_yaml_config(paths: &AppPaths) -> FilesResult<FilesConfigYaml> {
        let config_path = paths.system().services().join("config/files.yaml");

        if !config_path.exists() {
            return Ok(FilesConfigYaml::default());
        }

        let content =
            std::fs::read_to_string(&config_path).map_err(|source| FilesError::ConfigRead {
                path: config_path.clone(),
                source,
            })?;

        let wrapper: FilesConfigWrapper =
            serde_yaml::from_str(&content).map_err(|source| FilesError::ConfigParse {
                path: config_path,
                source,
            })?;

        Ok(wrapper.files)
    }

    pub const fn upload(&self) -> &FileUploadConfig {
        &self.upload
    }

    pub const fn documents_config(&self) -> &DocumentsConfig {
        &self.documents
    }

    pub fn validate(&self) -> FilesResult<()> {
        if !self.storage_root.is_absolute() {
            return Err(FilesError::Config(format!(
                "paths.storage must be absolute, got: {}",
                self.storage_root.display()
            )));
        }
        Ok(())
    }

    pub fn ensure_storage_structure(&self) -> FilesResult<()> {
        for dir in [self.storage_root.clone(), self.files(), self.images()] {
            if !dir.exists() {
                std::fs::create_dir_all(&dir).map_err(|source| FilesError::StorageDir {
                    path: dir.clone(),
                    source,
                })?;
            }
        }
        Ok(())
    }

    pub fn storage(&self) -> &Path {
        &self.storage_root
    }

    pub fn generated_images(&self) -> PathBuf {
        self.storage_root.join(storage::GENERATED)
    }

    pub fn content_images(&self, source: &str) -> PathBuf {
        self.storage_root.join(storage::IMAGES).join(source)
    }

    pub fn images(&self) -> PathBuf {
        self.storage_root.join(storage::IMAGES)
    }

    pub fn files(&self) -> PathBuf {
        self.storage_root.join(storage::FILES)
    }

    pub fn audio(&self) -> PathBuf {
        self.storage_root.join(storage::AUDIO)
    }

    pub fn video(&self) -> PathBuf {
        self.storage_root.join(storage::VIDEO)
    }

    pub fn documents(&self) -> PathBuf {
        self.storage_root.join(storage::DOCUMENTS)
    }

    pub fn uploads(&self) -> PathBuf {
        self.storage_root.join(storage::UPLOADS)
    }

    pub fn storage_root(&self) -> &Path {
        &self.storage_root
    }

    pub fn uploads_relative() -> PathBuf {
        PathBuf::from(storage::UPLOADS)
    }

    pub fn generated_images_relative() -> PathBuf {
        PathBuf::from(storage::GENERATED)
    }

    pub fn url_prefix(&self) -> &str {
        &self.url_prefix
    }

    pub fn cache_control(&self) -> Option<&str> {
        self.cache_control.as_deref()
    }

    pub fn public_url(&self, relative_path: &str) -> String {
        let path = relative_path.trim_start_matches('/');
        format!("{}/{}", self.url_prefix, path)
    }

    pub fn image_url(&self, relative_to_images: &str) -> String {
        let path = relative_to_images.trim_start_matches('/');
        format!("{}/images/{}", self.url_prefix, path)
    }

    pub fn generated_image_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/images/generated/{}", self.url_prefix, name)
    }

    pub fn content_image_url(&self, source: &str, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/images/{}/{}", self.url_prefix, source, name)
    }

    pub fn file_url(&self, relative_to_files: &str) -> String {
        let path = relative_to_files.trim_start_matches('/');
        format!("{}/files/{}", self.url_prefix, path)
    }

    pub fn audio_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/audio/{}", self.url_prefix, name)
    }

    pub fn video_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/video/{}", self.url_prefix, name)
    }

    pub fn document_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/documents/{}", self.url_prefix, name)
    }

    pub fn upload_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/uploads/{}", self.url_prefix, name)
    }
}
