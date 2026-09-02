//! [`FileUploadService`]: decode, validate, store, and record uploads.
//!
//! Decodes base64 payloads, enforces upload policy via [`FileValidator`],
//! writes bytes through the [`FileStorage`] backend at a traversal-checked
//! path derived from the persistence mode, and records the file through
//! [`FileRepository`], deleting the stored artefact if the database write
//! fails.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, FileId, UserId};
use systemprompt_traits::{FileStorage, StoredFileId};
use uuid::Uuid;

use crate::config::{FilePersistenceMode, FilesConfig};
use crate::models::{FileChecksums, FileMetadata};
use crate::repository::{FileRepository, InsertFileRequest};

use super::error::FileUploadError;
use super::request::{FileUploadRequest, UploadedFile};
use super::validator::{FileCategory, FileValidator};

struct StoredArtifact<'a> {
    file_id: &'a FileId,
    stored_id: &'a StoredFileId,
    public_url: &'a str,
    size_bytes: u64,
    sha256: String,
}

#[derive(Clone)]
pub struct FileUploadService {
    files_config: FilesConfig,
    file_repository: FileRepository,
    validator: FileValidator,
    storage: Arc<dyn FileStorage>,
}

impl std::fmt::Debug for FileUploadService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileUploadService")
            .field("files_config", &self.files_config)
            .field("validator", &self.validator)
            .finish_non_exhaustive()
    }
}

impl FileUploadService {
    pub fn new(
        file_repository: FileRepository,
        files_config: FilesConfig,
        storage: Arc<dyn FileStorage>,
    ) -> Self {
        let validator = FileValidator::new(*files_config.upload());

        Self {
            files_config,
            file_repository,
            validator,
            storage,
        }
    }

    pub const fn validator(&self) -> &FileValidator {
        &self.validator
    }

    pub fn is_enabled(&self) -> bool {
        let cfg = self.files_config.upload();
        cfg.enabled && cfg.persistence_mode != FilePersistenceMode::Disabled
    }

    pub async fn upload_file(
        &self,
        request: FileUploadRequest,
    ) -> Result<UploadedFile, FileUploadError> {
        let upload_config = self.files_config.upload();

        if upload_config.persistence_mode == FilePersistenceMode::Disabled {
            return Err(FileUploadError::PersistenceDisabled);
        }

        let max_encoded_size = (upload_config.max_file_size_bytes as f64 * 1.34) as usize + 100;
        if request.bytes_base64.len() > max_encoded_size {
            return Err(FileUploadError::Base64TooLarge {
                encoded_size: request.bytes_base64.len(),
            });
        }

        let bytes = STANDARD.decode(&request.bytes_base64)?;
        let size_bytes = bytes.len() as u64;

        let category = self.validator.validate(&request.mime_type, size_bytes)?;

        let file_id = FileId::new(Uuid::new_v4().to_string());
        let extension = FileValidator::get_extension(&request.mime_type, request.name.as_deref());
        let filename = format!("{}.{}", file_id.as_str(), extension);

        let (storage_path, relative_path) = self.determine_storage_path(
            &category,
            &filename,
            &request.context_id,
            request.user_id.as_ref(),
        )?;

        let stored_id = self.storage.store(&storage_path, &bytes).await?;

        let sha256 = hex::encode(Sha256::digest(&bytes));

        let public_url = self.files_config.upload_url(&relative_path);

        self.record_file(
            StoredArtifact {
                file_id: &file_id,
                stored_id: &stored_id,
                public_url: &public_url,
                size_bytes,
                sha256,
            },
            &request,
        )
        .await?;

        Ok(UploadedFile {
            file_id,
            path: relative_path,
            public_url,
            size_bytes: size_bytes as i64,
        })
    }

    async fn record_file(
        &self,
        artifact: StoredArtifact<'_>,
        request: &FileUploadRequest,
    ) -> Result<(), FileUploadError> {
        let StoredArtifact {
            file_id,
            stored_id,
            public_url,
            size_bytes,
            sha256,
        } = artifact;

        let metadata = FileMetadata::new().with_checksums(FileChecksums::new().with_sha256(sha256));

        let mut insert_request = InsertFileRequest::new(
            file_id.clone(),
            stored_id.as_str().to_owned(),
            public_url.to_owned(),
            request.mime_type.clone(),
        )
        .with_size(size_bytes as i64)
        .with_metadata(metadata)
        .with_context_id(request.context_id.clone());

        if let Some(user_id) = request.user_id.clone() {
            insert_request = insert_request.with_user_id(user_id);
        }

        if let Some(session_id) = request.session_id.clone() {
            insert_request = insert_request.with_session_id(session_id);
        }

        if let Some(trace_id) = request.trace_id.clone() {
            insert_request = insert_request.with_trace_id(trace_id);
        }

        if let Err(e) = self.file_repository.insert(insert_request).await {
            if let Err(cleanup_err) = self.storage.delete(stored_id).await {
                tracing::warn!(
                    stored_id = %stored_id,
                    error = %cleanup_err,
                    "Failed to clean up uploaded file after database error"
                );
            }
            return Err(FileUploadError::Database(e.to_string()));
        }

        Ok(())
    }

    fn determine_storage_path(
        &self,
        category: &FileCategory,
        filename: &str,
        context_id: &ContextId,
        user_id: Option<&UserId>,
    ) -> Result<(PathBuf, String), FileUploadError> {
        let base = FilesConfig::uploads_relative();
        let upload_config = self.files_config.upload();

        let context_str = context_id.as_str();
        Self::validate_path_inputs(context_str, filename, user_id)?;

        let (full_path, relative) = match upload_config.persistence_mode {
            FilePersistenceMode::ContextScoped => {
                let rel = format!(
                    "contexts/{}/{}/{}",
                    context_str,
                    category.storage_subdir(),
                    filename
                );
                (base.join(&rel), rel)
            },
            FilePersistenceMode::UserLibrary => {
                let user_dir =
                    user_id.map_or_else(|| "anonymous".to_owned(), |u| u.as_str().to_owned());
                let rel = format!(
                    "users/{}/{}/{}",
                    user_dir,
                    category.storage_subdir(),
                    filename
                );
                (base.join(&rel), rel)
            },
            FilePersistenceMode::Disabled => {
                return Err(FileUploadError::PersistenceDisabled);
            },
        };

        for component in std::path::Path::new(&relative).components() {
            use std::path::Component;
            match component {
                Component::Normal(_) | Component::CurDir => {},
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(FileUploadError::PathValidation(
                        "Resolved path contains traversal or absolute component".to_owned(),
                    ));
                },
            }
        }

        Ok((full_path, relative))
    }

    fn validate_path_inputs(
        context_str: &str,
        filename: &str,
        user_id: Option<&UserId>,
    ) -> Result<(), FileUploadError> {
        if context_str.contains("..") || context_str.contains('\0') {
            return Err(FileUploadError::PathValidation(
                "Invalid context_id: contains path traversal sequence".to_owned(),
            ));
        }

        if let Some(uid) = user_id {
            let user_str = uid.as_str();
            if user_str.contains("..") || user_str.contains('\0') {
                return Err(FileUploadError::PathValidation(
                    "Invalid user_id: contains path traversal sequence".to_owned(),
                ));
            }
        }

        if filename.contains('\0')
            || filename.contains('/')
            || filename.contains('\\')
            || filename == ".."
            || filename == "."
            || filename.is_empty()
        {
            return Err(FileUploadError::PathValidation(
                "Invalid filename: must be a single path component".to_owned(),
            ));
        }

        Ok(())
    }
}
