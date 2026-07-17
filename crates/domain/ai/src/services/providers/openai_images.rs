//! `OpenAI` image-generation provider.
//!
//! [`OpenAiImageProvider`] implements the `ImageProvider` trait against the
//! `OpenAI` `images/generations` endpoint (gpt-image / DALL·E models), mapping
//! the platform's aspect ratios onto the API's fixed size options.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, Result};
use crate::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageGenerationResponse, ImageResolution,
    NewImageGenerationResponse,
};
use crate::services::providers::image_provider_trait::{
    ImageProvider, ImageProviderCapabilities, registry_image_models, registry_per_image_cents,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use systemprompt_models::net::IMAGE_GEN_OPENAI_TIMEOUT;
use systemprompt_models::services::ModelDefinition;

const DEFAULT_IMAGE_CENTS: f32 = 4.0;

#[derive(Debug)]
pub struct OpenAiImageProvider {
    client: Client,
    api_key: String,
    endpoint: String,
    default_model: String,
    model_definitions: HashMap<String, ModelDefinition>,
}

impl OpenAiImageProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(IMAGE_GEN_OPENAI_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Failed to build OpenAI image HTTP client, falling back to default client");
                Client::new()
            });

        Self {
            client,
            api_key,
            endpoint: "https://api.openai.com/v1".to_owned(),
            default_model: "gpt-image-1".to_owned(),
            model_definitions: HashMap::new(),
        }
    }

    pub fn with_endpoint(api_key: String, endpoint: String) -> Self {
        let mut provider = Self::new(api_key);
        provider.endpoint = endpoint;
        provider
    }

    pub fn with_default_model(mut self, model: String) -> Self {
        self.default_model = model;
        self
    }

    pub fn with_model_definitions(mut self, models: HashMap<String, ModelDefinition>) -> Self {
        self.model_definitions = models;
        self
    }

    const fn map_size(aspect_ratio: &AspectRatio) -> &'static str {
        match aspect_ratio {
            AspectRatio::Square => "1024x1024",
            AspectRatio::Portrait916 | AspectRatio::Portrait34 => "1024x1792",
            _ => "1792x1024",
        }
    }

    fn validate_request(&self, request: &ImageGenerationRequest) -> Result<()> {
        if request.prompt.len() > self.capabilities().max_prompt_length {
            return Err(AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!(
                    "Prompt length {} exceeds maximum {}",
                    request.prompt.len(),
                    self.capabilities().max_prompt_length
                ),
            });
        }

        if !self.supports_resolution(&request.resolution) {
            return Err(AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!("Resolution {} not supported", request.resolution.as_str()),
            });
        }

        if !self.supports_aspect_ratio(&request.aspect_ratio) {
            return Err(AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!(
                    "Aspect ratio {} not supported",
                    request.aspect_ratio.as_str()
                ),
            });
        }

        Ok(())
    }

    async fn request_image(&self, dalle_request: &DalleRequest) -> Result<String> {
        let url = format!("{}/images/generations", self.endpoint);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(dalle_request)
            .send()
            .await
            .map_err(AiError::Http)?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<error reading response: {}>", e));
            return Err(AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!("API returned status {status}: {error_body}"),
            });
        }

        let dalle_response: DalleResponse = response.json().await.map_err(AiError::Http)?;

        dalle_response
            .data
            .first()
            .and_then(|d| d.b64_json.clone())
            .ok_or_else(|| AiError::ProviderError {
                provider: self.name().to_owned(),
                message: "No image data in response".to_owned(),
            })
    }
}

#[derive(Debug, Serialize)]
struct DalleRequest {
    model: String,
    prompt: String,
    size: String,
    quality: String,
    n: u32,
    response_format: String,
}

#[derive(Debug, Deserialize)]
struct DalleResponse {
    data: Vec<DalleImageData>,
}

#[derive(Debug, Deserialize)]
struct DalleImageData {
    b64_json: Option<String>,
}

#[async_trait]
impl ImageProvider for OpenAiImageProvider {
    fn name(&self) -> &'static str {
        "openai-image"
    }

    fn capabilities(&self) -> ImageProviderCapabilities {
        ImageProviderCapabilities {
            supported_resolutions: vec![ImageResolution::OneK],
            supported_aspect_ratios: vec![
                AspectRatio::Square,
                AspectRatio::Landscape169,
                AspectRatio::Portrait916,
            ],
            supports_batch: false,
            supports_image_editing: true,
            supports_search_grounding: false,
            max_prompt_length: 4000,
            cost_per_image_cents: registry_per_image_cents(
                &self.model_definitions,
                &self.default_model,
                DEFAULT_IMAGE_CENTS,
            ),
        }
    }

    fn supported_models(&self) -> Vec<String> {
        let models = registry_image_models(&self.model_definitions);
        if models.is_empty() {
            vec![self.default_model.clone()]
        } else {
            models
        }
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    async fn generate_image(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse> {
        let start = Instant::now();

        self.validate_request(request)?;

        let model = request
            .model
            .as_deref()
            .unwrap_or_else(|| self.default_model());

        if !self.supports_model(model) {
            return Err(AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!("Model {model} not supported"),
            });
        }

        let dalle_request = DalleRequest {
            model: model.to_owned(),
            prompt: request.prompt.clone(),
            size: Self::map_size(&request.aspect_ratio).to_owned(),
            quality: "standard".to_owned(),
            n: 1,
            response_format: "b64_json".to_owned(),
        };

        let image_data = self.request_image(&dalle_request).await?;

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(ImageGenerationResponse::new(NewImageGenerationResponse {
            provider: self.name().to_owned(),
            model: model.to_owned(),
            image_data,
            mime_type: "image/png".to_owned(),
            resolution: request.resolution,
            aspect_ratio: request.aspect_ratio,
            generation_time_ms,
        }))
    }

    async fn generate_batch(
        &self,
        requests: &[ImageGenerationRequest],
    ) -> Result<Vec<ImageGenerationResponse>> {
        let mut responses = Vec::new();
        for request in requests {
            responses.push(self.generate_image(request).await?);
        }
        Ok(responses)
    }
}
