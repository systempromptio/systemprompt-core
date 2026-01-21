use crate::error::AiError;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_traits::ImageStorageConfig;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub base_path: PathBuf,
    pub url_prefix: String,
    #[serde(default = "default_max_file_size")]
    pub max_file_size_bytes: usize,
    #[serde(default = "default_organize_by_date")]
    pub organize_by_date: bool,
}

const fn default_max_file_size() -> usize {
    10 * 1024 * 1024
}

const fn default_organize_by_date() -> bool {
    true
}

impl StorageConfig {
    pub fn from_image_storage_config(config: ImageStorageConfig) -> Self {
        Self {
            base_path: config.base_path,
            url_prefix: config.url_prefix,
            max_file_size_bytes: default_max_file_size(),
            organize_by_date: true,
        }
    }
}

impl StorageConfig {
    pub const fn new(base_path: PathBuf, url_prefix: String) -> Self {
        Self {
            base_path,
            url_prefix,
            max_file_size_bytes: default_max_file_size(),
            organize_by_date: default_organize_by_date(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.url_prefix.is_empty() {
            return Err("url_prefix cannot be empty".to_string());
        }

        if self.max_file_size_bytes == 0 {
            return Err("max_file_size_bytes must be greater than 0".to_string());
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct ImageStorage {
    config: StorageConfig,
}

impl ImageStorage {
    pub fn new(config: StorageConfig) -> Result<Self, AiError> {
        config
            .validate()
            .map_err(|e| AiError::StorageError(format!("Invalid storage configuration: {e}")))?;

        if !config.base_path.exists() {
            fs::create_dir_all(&config.base_path).map_err(|e| {
                AiError::StorageError(format!(
                    "Failed to create storage directory {}: {}",
                    config.base_path.display(),
                    e
                ))
            })?;
        }

        Ok(Self { config })
    }

    pub fn save_base64_image(
        &self,
        base64_data: &str,
        mime_type: &str,
    ) -> Result<(PathBuf, String), AiError> {
        let image_bytes = BASE64
            .decode(base64_data)
            .map_err(|e| AiError::StorageError(format!("Failed to decode base64 image: {e}")))?;

        self.save_image_bytes(&image_bytes, mime_type)
    }

    pub fn save_image_bytes(
        &self,
        image_bytes: &[u8],
        mime_type: &str,
    ) -> Result<(PathBuf, String), AiError> {
        if image_bytes.len() > self.config.max_file_size_bytes {
            return Err(AiError::StorageError(format!(
                "Image size {} bytes exceeds maximum allowed size {} bytes",
                image_bytes.len(),
                self.config.max_file_size_bytes
            )));
        }

        let extension = Self::mime_type_to_extension(mime_type);
        let filename = format!(
            "{}_{}.{}",
            Uuid::new_v4(),
            Utc::now().timestamp(),
            extension
        );

        let relative_path = if self.config.organize_by_date {
            let now = Utc::now();
            PathBuf::from(format!(
                "{}/{:04}/{:02}/{:02}/{}",
                self.config.base_path.display(),
                now.year(),
                now.month(),
                now.day(),
                filename
            ))
        } else {
            self.config.base_path.join(&filename)
        };

        if let Some(parent) = relative_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    AiError::StorageError(format!(
                        "Failed to create directory {}: {e}",
                        parent.display()
                    ))
                })?;
            }
        }

        fs::write(&relative_path, image_bytes).map_err(|e| {
            AiError::StorageError(format!(
                "Failed to write image file {}: {e}",
                relative_path.display()
            ))
        })?;

        let url_path = if self.config.organize_by_date {
            let now = Utc::now();
            format!(
                "{}/{:04}/{:02}/{:02}/{}",
                self.config.url_prefix,
                now.year(),
                now.month(),
                now.day(),
                filename
            )
        } else {
            format!("{}/{}", self.config.url_prefix, filename)
        };

        Ok((relative_path, url_path))
    }

    pub fn delete_image(&self, file_path: &Path) -> Result<(), AiError> {
        if !file_path.exists() {
            return Err(AiError::StorageError(format!(
                "File does not exist: {}",
                file_path.display()
            )));
        }

        fs::remove_file(file_path).map_err(|e| {
            AiError::StorageError(format!(
                "Failed to delete file {}: {e}",
                file_path.display()
            ))
        })?;

        if let Some(parent) = file_path.parent() {
            let _ = self.cleanup_empty_directories(parent);
        }

        Ok(())
    }

    pub fn exists(file_path: &Path) -> bool {
        file_path.exists()
    }

    pub fn get_full_path(&self, relative_path: &str) -> PathBuf {
        self.config.base_path.join(relative_path)
    }

    fn mime_type_to_extension(mime_type: &str) -> String {
        match mime_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "png",
        }
        .to_string()
    }

    fn cleanup_empty_directories(&self, dir: &Path) -> Result<(), std::io::Error> {
        if dir == self.config.base_path {
            return Ok(());
        }

        if dir.read_dir()?.next().is_none() {
            fs::remove_dir(dir)?;

            if let Some(parent) = dir.parent() {
                let _ = self.cleanup_empty_directories(parent);
            }
        }

        Ok(())
    }
}
