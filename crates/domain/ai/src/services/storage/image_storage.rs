//! Storage for generated images behind the [`FileStorage`] backend.
//!
//! [`ImageStorage`] persists decoded image bytes under a configured base path
//! relative to the storage root, optionally sharding by capture date, and
//! returns both the storage id and the public URL. [`StorageConfig`] holds
//! the base path, URL prefix, size cap, and date-organisation flag and
//! validates them before any write.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::AiError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_traits::{FileStorage, ImageStorageConfig, StoredFileId};
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
            return Err("url_prefix cannot be empty".to_owned());
        }

        if self.max_file_size_bytes == 0 {
            return Err("max_file_size_bytes must be greater than 0".to_owned());
        }

        Ok(())
    }
}

pub struct ImageStorage {
    config: StorageConfig,
    storage: Arc<dyn FileStorage>,
}

impl std::fmt::Debug for ImageStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageStorage")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

const fn storage_error(message: String) -> AiError {
    AiError::StorageError { message }
}

impl ImageStorage {
    pub fn new(config: StorageConfig, storage: Arc<dyn FileStorage>) -> Result<Self, AiError> {
        config
            .validate()
            .map_err(|e| storage_error(format!("Invalid storage configuration: {e}")))?;

        Ok(Self { config, storage })
    }

    pub async fn save_base64_image(
        &self,
        base64_data: &str,
        mime_type: &str,
    ) -> Result<(StoredFileId, String), AiError> {
        let image_bytes = BASE64
            .decode(base64_data)
            .map_err(|e| storage_error(format!("Failed to decode base64 image: {e}")))?;

        self.save_image_bytes(&image_bytes, mime_type).await
    }

    pub async fn save_image_bytes(
        &self,
        image_bytes: &[u8],
        mime_type: &str,
    ) -> Result<(StoredFileId, String), AiError> {
        if image_bytes.len() > self.config.max_file_size_bytes {
            return Err(storage_error(format!(
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

        let partition = self.date_partition();
        let relative_path = self.config.base_path.join(&partition).join(&filename);
        let url_path = format!(
            "{}/{}{}",
            self.config.url_prefix,
            if partition.is_empty() {
                String::new()
            } else {
                format!("{partition}/")
            },
            filename
        );

        let stored_id = self
            .storage
            .store(&relative_path, image_bytes)
            .await
            .map_err(|e| {
                storage_error(format!(
                    "Failed to write image file {}: {e}",
                    relative_path.display()
                ))
            })?;

        Ok((stored_id, url_path))
    }

    pub async fn delete_image(&self, id: &StoredFileId) -> Result<(), AiError> {
        self.storage
            .delete(id)
            .await
            .map_err(|e| storage_error(format!("Failed to delete file {id}: {e}")))
    }

    pub async fn exists(&self, id: &StoredFileId) -> Result<bool, AiError> {
        self.storage
            .exists(id)
            .await
            .map_err(|e| storage_error(format!("Failed to stat file {id}: {e}")))
    }

    pub fn get_full_path(&self, relative_path: &str) -> PathBuf {
        self.config.base_path.join(relative_path)
    }

    fn date_partition(&self) -> String {
        if !self.config.organize_by_date {
            return String::new();
        }
        let now = Utc::now();
        format!("{:04}/{:02}/{:02}", now.year(), now.month(), now.day())
    }

    fn mime_type_to_extension(mime_type: &str) -> String {
        match mime_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "png",
        }
        .to_owned()
    }
}
