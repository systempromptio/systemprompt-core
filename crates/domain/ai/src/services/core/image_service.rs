use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageGenerationResponse};
use crate::models::AiRequestRecordBuilder;
use crate::repository::AiRequestRepository;
use crate::services::providers::image_provider_trait::BoxedImageProvider;
use crate::services::storage::{ImageStorage, StorageConfig};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_core_database::DbPool;
use systemprompt_core_files::{
    File, FileMetadata, FileRepository, ImageGenerationInfo, ImageMetadata,
};
use systemprompt_identifiers::{FileId, McpExecutionId, SessionId, TraceId, UserId};
use tracing::error;
use uuid::Uuid;

pub struct ImageService {
    providers: HashMap<String, BoxedImageProvider>,
    storage: Arc<ImageStorage>,
    file_repo: FileRepository,
    ai_request_repo: AiRequestRepository,
    default_provider: Option<String>,
}

impl std::fmt::Debug for ImageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageService")
            .field("providers", &format!("{} providers", self.providers.len()))
            .field("storage", &self.storage)
            .field("file_repo", &"FileRepository")
            .field("ai_request_repo", &"AiRequestRepository")
            .field("default_provider", &self.default_provider)
            .finish()
    }
}

impl ImageService {
    pub fn new(db_pool: &DbPool, storage_config: StorageConfig) -> Result<Self> {
        let storage = Arc::new(ImageStorage::new(storage_config)?);
        let file_repo = FileRepository::new(db_pool)?;
        let ai_request_repo = AiRequestRepository::new(db_pool)?;

        Ok(Self {
            providers: HashMap::new(),
            storage,
            file_repo,
            ai_request_repo,
            default_provider: None,
        })
    }

    pub fn with_providers(
        db_pool: &DbPool,
        storage_config: StorageConfig,
        providers: HashMap<String, BoxedImageProvider>,
        default_provider: Option<String>,
    ) -> Result<Self> {
        let storage = Arc::new(ImageStorage::new(storage_config)?);
        let file_repo = FileRepository::new(db_pool)?;
        let ai_request_repo = AiRequestRepository::new(db_pool)?;

        Ok(Self {
            providers,
            storage,
            file_repo,
            ai_request_repo,
            default_provider,
        })
    }

    pub fn register_provider(&mut self, provider: BoxedImageProvider) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    pub fn set_default_provider(&mut self, provider_name: String) {
        self.default_provider = Some(provider_name);
    }

    pub fn get_provider(&self, name: &str) -> Option<&BoxedImageProvider> {
        self.providers.get(name)
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub async fn generate_image(
        &self,
        mut request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse> {
        let provider_name = if let Some(model) = &request.model {
            self.find_provider_for_model(model)?
        } else if let Some(default) = &self.default_provider {
            default.clone()
        } else {
            return Err(AiError::ConfigurationError(
                "No model specified and no default provider configured".to_string(),
            ));
        };

        let provider =
            self.providers
                .get(&provider_name)
                .ok_or_else(|| AiError::ProviderError {
                    provider: provider_name.clone(),
                    message: "Provider not found".to_string(),
                })?;

        if request.trace_id.is_none() {
            request.trace_id = Some(Uuid::new_v4().to_string());
        }

        let generation_result = provider.generate_image(&request).await;

        let mut response = match generation_result {
            Ok(resp) => resp,
            Err(e) => {
                error!(
                    error = %e,
                    provider = %provider_name,
                    model = ?request.model,
                    prompt_preview = %request.prompt.chars().take(200).collect::<String>(),
                    prompt_length = request.prompt.len(),
                    resolution = %request.resolution.as_str(),
                    aspect_ratio = %request.aspect_ratio.as_str(),
                    trace_id = ?request.trace_id,
                    user_id = ?request.user_id,
                    session_id = ?request.session_id,
                    reference_images_count = request.reference_images.len(),
                    "Image generation failed - full request context logged for debugging"
                );
                return Err(e);
            },
        };
        response.cost_estimate = Some(provider.capabilities().cost_per_image_cents);

        let (file_path, public_url) = self
            .storage
            .save_base64_image(&response.image_data, &response.mime_type)?;

        response.file_path = Some(file_path.to_string_lossy().to_string());
        response.public_url = Some(public_url.clone());
        response.file_size_bytes = Some(response.image_data.len());

        self.persist_image_generation(
            &request,
            &response,
            &file_path.to_string_lossy(),
            &public_url,
        )
        .await?;

        Ok(response)
    }

    pub async fn generate_batch(
        &self,
        requests: Vec<ImageGenerationRequest>,
    ) -> Result<Vec<ImageGenerationResponse>> {
        let mut responses = Vec::new();

        for request in requests {
            match self.generate_image(request).await {
                Ok(response) => responses.push(response),
                Err(e) => {
                    return Err(e);
                },
            }
        }

        Ok(responses)
    }

    pub async fn get_generated_image(&self, uuid: &str) -> Result<Option<File>> {
        self.file_repo
            .find_by_id(&FileId::new(uuid))
            .await
            .map_err(AiError::DatabaseError)
    }

    pub async fn list_user_images(
        &self,
        user_id: &UserId,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<File>> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);
        self.file_repo
            .list_by_user(user_id, limit, offset)
            .await
            .map_err(AiError::DatabaseError)
    }

    pub async fn delete_image(&self, uuid: &str) -> Result<()> {
        let file_id = FileId::new(uuid);
        let file = self.file_repo.find_by_id(&file_id).await?;

        if let Some(file_record) = file {
            let file_path = std::path::Path::new(&file_record.path);
            self.storage.delete_image(file_path)?;
            self.file_repo.delete(&file_id).await?;
        }

        Ok(())
    }

    fn find_provider_for_model(&self, model: &str) -> Result<String> {
        for (name, provider) in &self.providers {
            if provider.supports_model(model) {
                return Ok(name.clone());
            }
        }

        Err(AiError::ProviderError {
            provider: "unknown".to_string(),
            message: format!("No provider found for model: {model}"),
        })
    }

    async fn persist_image_generation(
        &self,
        request: &ImageGenerationRequest,
        response: &ImageGenerationResponse,
        file_path: &str,
        public_url: &str,
    ) -> Result<()> {
        let user_id = UserId::new(request.user_id.as_deref().unwrap_or("anonymous"));

        let mut builder = AiRequestRecordBuilder::new(&response.request_id, user_id)
            .provider(&response.provider)
            .model(&response.model)
            .cost(response.cost_estimate.map_or(0, |c| c.round() as i32))
            .latency(response.generation_time_ms as i32)
            .completed();

        if let Some(session_id) = &request.session_id {
            builder = builder.session_id(SessionId::new(session_id));
        }

        if let Some(trace_id) = &request.trace_id {
            builder = builder.trace_id(TraceId::new(trace_id));
        }

        if let Some(mcp_execution_id) = &request.mcp_execution_id {
            builder = builder.mcp_execution_id(McpExecutionId::new(mcp_execution_id));
        }

        let record = builder
            .build()
            .map_err(|e| AiError::InvalidInput(e.to_string()))?;

        self.ai_request_repo
            .insert(&record)
            .await
            .map_err(|e| AiError::DatabaseError(e.into()))?;

        let generation_info =
            ImageGenerationInfo::new(&request.prompt, &response.model, &response.provider)
                .with_resolution(response.resolution.as_str())
                .with_aspect_ratio(response.aspect_ratio.as_str())
                .with_generation_time(response.generation_time_ms as i32)
                .with_request_id(&response.request_id);

        let generation_info = match response.cost_estimate {
            Some(cost) => generation_info.with_cost_estimate(cost),
            None => generation_info,
        };

        let image_metadata = ImageMetadata::new().with_generation(generation_info);
        let metadata = serde_json::to_value(FileMetadata::new().with_image(image_metadata))
            .map_err(AiError::SerializationError)?;

        let now = Utc::now();
        let file = File {
            id: Uuid::parse_str(&response.id)
                .map_err(|e| AiError::InvalidInput(format!("Invalid UUID: {e}")))?,
            path: file_path.to_string(),
            public_url: public_url.to_string(),
            mime_type: response.mime_type.clone(),
            size_bytes: response.file_size_bytes.map(|s| s as i64),
            ai_content: true,
            metadata,
            user_id: request.user_id.as_ref().map(UserId::new),
            session_id: request.session_id.as_ref().map(SessionId::new),
            trace_id: request.trace_id.as_ref().map(TraceId::new),
            context_id: None,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };

        self.file_repo.insert_file(&file).await.map_err(|e| {
            AiError::DatabaseError(anyhow::anyhow!(
                "Failed to persist generated image (id: {}, path: {}, url: {}): {}",
                response.id,
                file_path,
                public_url,
                e
            ))
        })?;

        Ok(())
    }
}
