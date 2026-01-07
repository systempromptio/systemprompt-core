use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;

static FILES_CONFIG: OnceLock<FilesConfig> = OnceLock::new();

const DEFAULT_URL_PREFIX: &str = "/files";

#[derive(Debug, Clone)]
pub struct FilesConfig {
    storage_root: PathBuf,
    url_prefix: String,
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

        Ok(Self {
            storage_root: PathBuf::from(storage_root),
            url_prefix: DEFAULT_URL_PREFIX.to_string(),
        })
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
            return errors;
        }

        let required_image_subdirs = ["blog", "social", "logos"];
        for subdir in required_image_subdirs {
            let path = images_dir.join(subdir);
            if !path.exists() {
                errors.push(format!(
                    "Required images subdirectory not found: {}",
                    path.display()
                ));
            }
        }

        errors
    }

    pub fn storage(&self) -> &Path {
        &self.storage_root
    }

    pub fn generated_images(&self) -> PathBuf {
        self.storage_root.join("images/generated")
    }

    /// Get the path to images for a content source (e.g., "blog", "docs", etc.)
    pub fn content_images(&self, source: &str) -> PathBuf {
        self.storage_root.join(format!("images/{}", source))
    }

    pub fn images(&self) -> PathBuf {
        self.storage_root.join("images")
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

    /// Get the URL for an image in a content source's directory
    pub fn content_image_url(&self, source: &str, filename: &str) -> String {
        let name = filename.trim_start_matches('/');
        format!("{}/images/{}/{}", self.url_prefix, source, name)
    }
}
