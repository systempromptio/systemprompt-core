mod types;
mod validator;

pub use types::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};
pub use validator::FilesConfigValidator;

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use systemprompt_cloud::constants::storage;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::AppPaths;

use types::FilesConfigWrapper;

static FILES_CONFIG: OnceLock<FilesConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct FilesConfig {
    storage_root: PathBuf,
    url_prefix: String,
    upload: FileUploadConfig,
}

impl FilesConfig {
    pub fn init() -> Result<()> {
        if FILES_CONFIG.get().is_some() {
            return Ok(());
        }
        let config = Self::from_profile()?;
        config.validate()?;
        let _ = FILES_CONFIG.set(config);
        Ok(())
    }

    pub fn get() -> Result<&'static Self> {
        FILES_CONFIG
            .get()
            .ok_or_else(|| anyhow!("FilesConfig::init() not called"))
    }

    pub fn get_optional() -> Option<&'static Self> {
        FILES_CONFIG.get()
    }

    pub fn from_profile() -> Result<Self> {
        let profile =
            ProfileBootstrap::get().map_err(|e| anyhow!("Profile not initialized: {}", e))?;

        let storage_root = profile
            .paths
            .storage
            .as_ref()
            .ok_or_else(|| anyhow!("paths.storage not configured in profile"))?
            .clone();

        let yaml_config = Self::load_yaml_config()?;

        Ok(Self {
            storage_root: PathBuf::from(storage_root),
            url_prefix: yaml_config.url_prefix,
            upload: yaml_config.upload,
        })
    }

    pub(crate) fn load_yaml_config() -> Result<FilesConfigYaml> {
        let paths = AppPaths::get().map_err(|e| anyhow!("{}", e))?;
        let config_path = paths.system().services().join("config/files.yaml");

        if !config_path.exists() {
            return Ok(FilesConfigYaml::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read files.yaml: {}", config_path.display()))?;

        let wrapper: FilesConfigWrapper = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse files.yaml: {}", config_path.display()))?;

        Ok(wrapper.files)
    }

    pub const fn upload(&self) -> &FileUploadConfig {
        &self.upload
    }

    pub fn validate(&self) -> Result<()> {
        if !self.storage_root.is_absolute() {
            return Err(anyhow!(
                "paths.storage must be absolute, got: {}",
                self.storage_root.display()
            ));
        }
        Ok(())
    }

    pub fn validate_storage_structure(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if !self.storage_root.exists() {
            errors.push(format!(
                "Storage root not found: {}",
                self.storage_root.display()
            ));
            return errors;
        }

        let images_dir = self.images();
        if !images_dir.exists() {
            errors.push(format!(
                "Images directory not found: {}",
                images_dir.display()
            ));
        }

        let files_dir = self.files();
        if !files_dir.exists() {
            errors.push(format!(
                "Files directory not found: {}",
                files_dir.display()
            ));
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
