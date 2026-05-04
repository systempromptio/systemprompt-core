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

/// Process-wide files configuration resolved from the active profile.
#[derive(Debug, Clone)]
pub struct FilesConfig {
    storage_root: PathBuf,
    url_prefix: String,
    upload: FileUploadConfig,
}

impl FilesConfig {
    /// Initialises the global [`FilesConfig`] from `paths` and the active
    /// profile. Idempotent.
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

    /// Returns the previously-initialised global config or
    /// [`FilesError::Config`].
    pub fn get() -> FilesResult<&'static Self> {
        FILES_CONFIG
            .get()
            .ok_or_else(|| FilesError::Config("FilesConfig::init() not called".into()))
    }

    /// Returns the global config if it has been initialised, else `None`.
    pub fn get_optional() -> Option<&'static Self> {
        FILES_CONFIG.get()
    }

    /// Builds a [`FilesConfig`] from the active profile and on-disk YAML
    /// overrides.
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

    pub(crate) fn load_yaml_config(paths: &AppPaths) -> FilesResult<FilesConfigYaml> {
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

    /// Returns the validated upload sub-config.
    pub const fn upload(&self) -> &FileUploadConfig {
        &self.upload
    }

    /// Validates the resolved config, currently asserting `storage_root` is
    /// absolute.
    pub fn validate(&self) -> FilesResult<()> {
        if !self.storage_root.is_absolute() {
            return Err(FilesError::Config(format!(
                "paths.storage must be absolute, got: {}",
                self.storage_root.display()
            )));
        }
        Ok(())
    }

    /// Ensures storage subdirectories exist; returns a list of human-readable
    /// error strings.
    pub fn ensure_storage_structure(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if !self.storage_root.exists() {
            if let Err(e) = std::fs::create_dir_all(&self.storage_root) {
                errors.push(format!(
                    "Failed to create storage root {}: {}",
                    self.storage_root.display(),
                    e
                ));
                return errors;
            }
        }

        for dir in [self.files(), self.images()] {
            if !dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    errors.push(format!("Failed to create {}: {}", dir.display(), e));
                }
            }
        }

        errors
    }

    /// Returns the absolute storage root.
    pub fn storage(&self) -> &Path {
        &self.storage_root
    }

    /// Returns the directory used for AI-generated images.
    pub fn generated_images(&self) -> PathBuf {
        self.storage_root.join(storage::GENERATED)
    }

    /// Returns the per-source content image directory.
    pub fn content_images(&self, source: &str) -> PathBuf {
        self.storage_root.join(storage::IMAGES).join(source)
    }

    /// Returns the root images directory.
    pub fn images(&self) -> PathBuf {
        self.storage_root.join(storage::IMAGES)
    }

    /// Returns the root files directory.
    pub fn files(&self) -> PathBuf {
        self.storage_root.join(storage::FILES)
    }

    /// Returns the audio storage directory.
    pub fn audio(&self) -> PathBuf {
        self.storage_root.join(storage::AUDIO)
    }

    /// Returns the video storage directory.
    pub fn video(&self) -> PathBuf {
        self.storage_root.join(storage::VIDEO)
    }

    /// Returns the document storage directory.
    pub fn documents(&self) -> PathBuf {
        self.storage_root.join(storage::DOCUMENTS)
    }

    /// Returns the user-upload directory.
    pub fn uploads(&self) -> PathBuf {
        self.storage_root.join(storage::UPLOADS)
    }

    /// Public URL prefix configured for served files.
    pub fn url_prefix(&self) -> &str {
        &self.url_prefix
    }

    /// Builds a public URL for a relative path under the storage root.
    pub fn public_url(&self, relative_path: &str) -> String {
        let path = relative_path.trim_start_matches('/');
        format!("{}/{}", self.url_prefix, path)
    }

    /// Builds a public URL for an asset under the images directory.
    pub fn image_url(&self, relative_to_images: &str) -> String {
        let path = relative_to_images.trim_start_matches('/');
        format!("{}/images/{}", self.url_prefix, path)
    }

    /// Builds a public URL for an AI-generated image.
    pub fn generated_image_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/images/generated/{}", self.url_prefix, name)
    }

    /// Builds a public URL for a per-source content image.
    pub fn content_image_url(&self, source: &str, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/images/{}/{}", self.url_prefix, source, name)
    }

    /// Builds a public URL for a generic file.
    pub fn file_url(&self, relative_to_files: &str) -> String {
        let path = relative_to_files.trim_start_matches('/');
        format!("{}/files/{}", self.url_prefix, path)
    }

    /// Builds a public URL for an audio asset.
    pub fn audio_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/audio/{}", self.url_prefix, name)
    }

    /// Builds a public URL for a video asset.
    pub fn video_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/video/{}", self.url_prefix, name)
    }

    /// Builds a public URL for a document asset.
    pub fn document_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/documents/{}", self.url_prefix, name)
    }

    /// Builds a public URL for a user upload.
    pub fn upload_url(&self, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/files/uploads/{}", self.url_prefix, name)
    }
}
