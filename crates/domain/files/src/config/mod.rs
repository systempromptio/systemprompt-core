//! Profile-driven configuration for the files crate.

mod types;
mod validator;

pub use types::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};
pub use validator::FilesConfigValidator;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use systemprompt_cloud::constants::storage;
use systemprompt_config::ProfileBootstrap;
use systemprompt_models::AppPaths;

use crate::error::{FilesError, FilesResult};
use types::FilesConfigWrapper;

static FILES_CONFIG: OnceLock<FilesConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct FilesConfig {
    storage_root: PathBuf,
    url_prefix: String,
    upload: FileUploadConfig,
}

impl FilesConfig {
    pub fn init(paths: &AppPaths) -> FilesResult<()> {
        if FILES_CONFIG.get().is_some() {
            return Ok(());
        }
        let config = Self::from_profile(paths)?;
        config.validate()?;
        if FILES_CONFIG.set(config).is_err() {
            tracing::warn!("FilesConfig was already initialized by a concurrent caller");
        }
        Ok(())
    }

    pub fn get() -> FilesResult<&'static Self> {
        FILES_CONFIG
            .get()
            .ok_or_else(|| FilesError::Config("FilesConfig::init() not called".into()))
    }

    pub fn get_optional() -> Option<&'static Self> {
        FILES_CONFIG.get()
    }

    pub fn from_profile(paths: &AppPaths) -> FilesResult<Self> {
        let profile = ProfileBootstrap::get()
            .map_err(|e| FilesError::Config(format!("Profile not initialized: {e}")))?;

        let storage_root = profile
            .paths
            .storage
            .as_ref()
            .ok_or_else(|| FilesError::Config("paths.storage not configured in profile".into()))?
            .clone();

        let yaml_config = Self::load_yaml_config(paths)?;

        Ok(Self {
            storage_root: PathBuf::from(storage_root),
            url_prefix: yaml_config.url_prefix,
            upload: yaml_config.upload,
        })
    }

    pub(super) fn load_yaml_config(paths: &AppPaths) -> FilesResult<FilesConfigYaml> {
        let config_path = paths.system().services().join("config/files.yaml");

        if !config_path.exists() {
            return Ok(FilesConfigYaml::default());
        }

        let content = std::fs::read_to_string(&config_path).map_err(|e| {
            FilesError::Config(format!(
                "Failed to read files.yaml ({}): {e}",
                config_path.display()
            ))
        })?;

        let wrapper: FilesConfigWrapper = serde_yaml::from_str(&content).map_err(|e| {
            FilesError::Config(format!(
                "Failed to parse files.yaml ({}): {e}",
                config_path.display()
            ))
        })?;

        Ok(wrapper.files)
    }

    pub const fn upload(&self) -> &FileUploadConfig {
        &self.upload
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

    pub fn ensure_storage_structure(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if !self.storage_root.exists()
            && let Err(e) = std::fs::create_dir_all(&self.storage_root)
        {
            errors.push(format!(
                "Failed to create storage root {}: {}",
                self.storage_root.display(),
                e
            ));
            return errors;
        }

        for dir in [self.files(), self.images()] {
            if !dir.exists()
                && let Err(e) = std::fs::create_dir_all(&dir)
            {
                errors.push(format!("Failed to create {}: {}", dir.display(), e));
            }
        }

        errors
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

    pub fn url_prefix(&self) -> &str {
        &self.url_prefix
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
