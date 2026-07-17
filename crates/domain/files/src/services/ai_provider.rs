//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{FileId, UserId};
use systemprompt_traits::{
    AiFilePersistenceProvider, AiGeneratedFile, AiProviderError, AiProviderResult,
    ImageStorageConfig, InsertAiFileParams,
};

use crate::config::FilesConfig;
use crate::error::FilesResult;
use crate::models::{File, FileMetadata};
use crate::repository::{FileRepository, InsertFileRequest};

fn to_ai_generated(f: File) -> AiGeneratedFile {
    AiGeneratedFile {
        id: f.id,
        path: f.path,
        public_url: f.public_url,
        mime_type: f.mime_type,
        size_bytes: f.size_bytes,
        ai_content: f.ai_content,
        metadata: serde_json::to_value(&f.metadata.0).unwrap_or_default(),
        user_id: f.user_id,
        session_id: f.session_id,
        trace_id: f.trace_id,
        context_id: f.context_id,
        created_at: f.created_at,
        updated_at: f.updated_at,
        deleted_at: f.deleted_at,
    }
}

#[derive(Debug)]
pub struct FilesAiPersistenceProvider {
    repository: FileRepository,
}

impl FilesAiPersistenceProvider {
    pub fn new(db: &DbPool) -> FilesResult<Self> {
        Ok(Self {
            repository: FileRepository::new(db)?,
        })
    }

    pub const fn from_repository(repository: FileRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl AiFilePersistenceProvider for FilesAiPersistenceProvider {
    async fn insert_file(&self, params: InsertAiFileParams) -> AiProviderResult<()> {
        let file_id = FileId::new(params.id.to_string());
        let metadata: FileMetadata = serde_json::from_value(params.metadata)
            .map_err(|e| AiProviderError::Internal(format!("Invalid file metadata: {e}")))?;
        let mut request =
            InsertFileRequest::new(file_id, params.path, params.public_url, params.mime_type)
                .with_ai_content(true)
                .with_metadata(metadata);

        if let Some(size) = params.size_bytes {
            request = request.with_size(size);
        }

        if let Some(user_id) = params.user_id {
            request = request.with_user_id(user_id);
        }

        if let Some(session_id) = params.session_id {
            request = request.with_session_id(session_id);
        }

        if let Some(trace_id) = params.trace_id {
            request = request.with_trace_id(trace_id);
        }

        if let Some(context_id) = params.context_id {
            request = request.with_context_id(context_id);
        }

        self.repository
            .insert(request)
            .await
            .map(|_| ())
            .map_err(|e| AiProviderError::Internal(e.to_string()))
    }

    async fn find_by_id(&self, id: &FileId) -> AiProviderResult<Option<AiGeneratedFile>> {
        let file = self
            .repository
            .find_by_id(id)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))?;

        Ok(file.map(to_ai_generated))
    }

    async fn list_by_user(
        &self,
        user_id: &UserId,
        limit: i64,
        offset: i64,
    ) -> AiProviderResult<Vec<AiGeneratedFile>> {
        let files = self
            .repository
            .list_by_user(user_id, limit, offset)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))?;

        Ok(files.into_iter().map(to_ai_generated).collect())
    }

    async fn delete(&self, id: &FileId) -> AiProviderResult<()> {
        self.repository
            .delete(id)
            .await
            .map_err(|e| AiProviderError::Internal(e.to_string()))
    }

    fn storage_config(&self) -> AiProviderResult<ImageStorageConfig> {
        let config = FilesConfig::get().map_err(|e| AiProviderError::ConfigurationError {
            message: e.to_string(),
        })?;

        Ok(ImageStorageConfig {
            base_path: config.generated_images(),
            url_prefix: format!("{}/images/generated", config.url_prefix()),
        })
    }
}
