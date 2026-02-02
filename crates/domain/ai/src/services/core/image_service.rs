use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageGenerationResponse};
use crate::repository::AiRequestRepository;
use crate::services::providers::image_provider_trait::{
    BoxedImageProvider, ImageProviderCapabilities,
};
use crate::services::storage::{ImageStorage, StorageConfig};
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_traits::{AiGeneratedFile, DynAiFilePersistenceProvider};
use tracing::error;
use uuid::Uuid;

use super::image_persistence;

pub struct ImageService {
    providers: HashMap<String, BoxedImageProvider>,
    storage: Arc<ImageStorage>,
    file_provider: DynAiFilePersistenceProvider,
    ai_request_repo: AiRequestRepository,
    default_provider: Option<String>,
}

impl std::fmt::Debug for ImageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageService")
            .field("providers", &format!("{} providers", self.providers.len()))
            .field("storage", &self.storage)
            .field("file_provider", &"AiFilePersistenceProvider")
            .field("ai_request_repo", &"AiRequestRepository")
            .field("default_provider", &self.default_provider)
            .finish()
    }
}

impl ImageService {
    pub fn new(
        db_pool: &DbPool,
        storage_config: StorageConfig,
        file_provider: DynAiFilePersistenceProvider,
    ) -> Result<Self> {
        let storage = Arc::new(ImageStorage::new(storage_config)?);
        let ai_request_repo = AiRequestRepository::new(db_pool)?;

        Ok(Self {
            providers: HashMap::new(),
            storage,
            file_provider,
            ai_request_repo,
            default_provider: None,
        })
    }

    pub fn with_providers(
        db_pool: &DbPool,
        storage_config: StorageConfig,
        file_provider: DynAiFilePersistenceProvider,
        providers: HashMap<String, BoxedImageProvider>,
        default_provider: Option<String>,
    ) -> Result<Self> {
        let storage = Arc::new(ImageStorage::new(storage_config)?);
        let ai_request_repo = AiRequestRepository::new(db_pool)?;

        Ok(Self {
            providers,
            storage,
            file_provider,
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

    pub fn default_provider_name(&self) -> Option<&str> {
        self.default_provider.as_deref()
    }

    pub fn get_default_provider(&self) -> Option<&BoxedImageProvider> {
        self.default_provider
            .as_ref()
            .and_then(|name| self.providers.get(name))
    }

    pub fn default_provider_capabilities(&self) -> Option<ImageProviderCapabilities> {
        self.get_default_provider().map(|p| p.capabilities())
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

        image_persistence::persist_image_generation(
            &self.ai_request_repo,
            self.file_provider.as_ref(),
            &request,
            &response,
            image_persistence::FileLocation {
                path: &file_path.to_string_lossy(),
                public_url: &public_url,
            },
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

    pub async fn get_generated_image(&self, uuid: &str) -> Result<Option<AiGeneratedFile>> {
        image_persistence::get_generated_image(self.file_provider.as_ref(), uuid).await
    }

    pub async fn list_user_images(
        &self,
        user_id: &UserId,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<AiGeneratedFile>> {
        image_persistence::list_user_images(self.file_provider.as_ref(), user_id, limit, offset)
            .await
    }

    pub async fn delete_image(&self, uuid: &str) -> Result<()> {
        image_persistence::delete_image(self.file_provider.as_ref(), &self.storage, uuid).await
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
}
