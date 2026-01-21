mod validator;

pub use validator::{FileCategory, FileValidationError, FileValidator};

use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::{FilePersistenceMode, FilesConfig};
use crate::models::{FileChecksums, FileMetadata};
use crate::repository::InsertFileRequest;
use crate::services::FileService;

#[derive(Debug, Error)]
pub enum FileUploadError {
    #[error("File persistence is disabled")]
    PersistenceDisabled,

    #[error("Validation failed: {0}")]
    Validation(#[from] FileValidationError),

    #[error("Failed to decode base64: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Clone)]
pub struct FileUploadRequest {
    pub name: Option<String>,
    pub mime_type: String,
    pub bytes_base64: String,
    pub context_id: ContextId,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug)]
pub struct FileUploadRequestBuilder {
    mime_type: String,
    bytes_base64: String,
    context_id: ContextId,
    name: Option<String>,
    user_id: Option<UserId>,
    session_id: Option<SessionId>,
    trace_id: Option<TraceId>,
}

impl FileUploadRequestBuilder {
    pub fn new(
        mime_type: impl Into<String>,
        bytes_base64: impl Into<String>,
        context_id: ContextId,
    ) -> Self {
        Self {
            mime_type: mime_type.into(),
            bytes_base64: bytes_base64.into(),
            context_id,
            name: None,
            user_id: None,
            session_id: None,
            trace_id: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_session_id(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = Some(trace_id);
        self
    }

    pub fn build(self) -> FileUploadRequest {
        FileUploadRequest {
            name: self.name,
            mime_type: self.mime_type,
            bytes_base64: self.bytes_base64,
            context_id: self.context_id,
            user_id: self.user_id,
            session_id: self.session_id,
            trace_id: self.trace_id,
        }
    }
}

impl FileUploadRequest {
    pub fn builder(
        mime_type: impl Into<String>,
        bytes_base64: impl Into<String>,
        context_id: ContextId,
    ) -> FileUploadRequestBuilder {
        FileUploadRequestBuilder::new(mime_type, bytes_base64, context_id)
    }
}

#[derive(Debug, Clone)]
pub struct UploadedFile {
    pub file_id: FileId,
    pub path: String,
    pub public_url: String,
    pub size_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct FileUploadService {
    files_config: FilesConfig,
    file_service: FileService,
    validator: FileValidator,
}

impl FileUploadService {
    pub fn new(db_pool: &DbPool, files_config: FilesConfig) -> Result<Self, FileUploadError> {
        let upload_config = *files_config.upload();
        let file_service =
            FileService::new(db_pool).map_err(|e| FileUploadError::Database(e.to_string()))?;
        let validator = FileValidator::new(upload_config);

        Ok(Self {
            files_config,
            file_service,
            validator,
        })
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

        if let Some(parent) = storage_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::File::create(&storage_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        let sha256 = hex::encode(Sha256::digest(&bytes));

        let public_url = self.files_config.upload_url(&relative_path);

        let metadata = FileMetadata::new().with_checksums(FileChecksums::new().with_sha256(sha256));

        let metadata_json = serde_json::to_value(&metadata)
            .map_err(|e| FileUploadError::Database(format!("Failed to serialize metadata: {e}")))?;

        let mut insert_request = InsertFileRequest::new(
            file_id.clone(),
            storage_path.to_string_lossy().to_string(),
            public_url.clone(),
            request.mime_type.clone(),
        )
        .with_size(size_bytes as i64)
        .with_metadata(metadata_json)
        .with_context_id(request.context_id.clone());

        if let Some(user_id) = request.user_id {
            insert_request = insert_request.with_user_id(user_id);
        }

        if let Some(session_id) = request.session_id {
            insert_request = insert_request.with_session_id(session_id);
        }

        if let Some(trace_id) = request.trace_id {
            insert_request = insert_request.with_trace_id(trace_id);
        }

        self.file_service
            .insert(insert_request)
            .await
            .map_err(|e| FileUploadError::Database(e.to_string()))?;

        Ok(UploadedFile {
            file_id,
            path: relative_path,
            public_url,
            size_bytes: size_bytes as i64,
        })
    }

    fn determine_storage_path(
        &self,
        category: &FileCategory,
        filename: &str,
        context_id: &ContextId,
        user_id: Option<&UserId>,
    ) -> Result<(PathBuf, String), FileUploadError> {
        let base = self.files_config.uploads();
        let upload_config = self.files_config.upload();

        let (full_path, relative) = match upload_config.persistence_mode {
            FilePersistenceMode::ContextScoped => {
                let rel = format!(
                    "contexts/{}/{}/{}",
                    context_id.as_str(),
                    category.storage_subdir(),
                    filename
                );
                (base.join(&rel), rel)
            },
            FilePersistenceMode::UserLibrary => {
                let user_dir =
                    user_id.map_or_else(|| "anonymous".to_string(), |u| u.as_str().to_string());
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

        Ok((full_path, relative))
    }
}
