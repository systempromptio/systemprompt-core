//! Gemini image-generation provider.
//!
//! [`GeminiImageProvider`] implements the `ImageProvider` trait against
//! Google's `generateContent` image models, validating prompt length,
//! resolution, and aspect ratio against its declared capabilities before
//! dispatching the request.

use crate::error::{AiError, Result};
use crate::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageGenerationResponse, ImageResolution,
    NewImageGenerationResponse,
};
use crate::models::providers::gemini::{GeminiRequest, GeminiResponse};
use crate::services::providers::image_provider_trait::{
    ImageProvider, ImageProviderCapabilities, registry_image_models, registry_per_image_cents,
};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Instant;
use systemprompt_models::net::{HTTP_STREAM_CONNECT_TIMEOUT, IMAGE_GEN_LONG_POLL_TIMEOUT};
use systemprompt_models::services::ModelDefinition;
use tracing::error;

use super::gemini_image_mapping::{build_image_request, extract_image_from_response};

const DEFAULT_IMAGE_CENTS: f32 = 4.0;

#[derive(Debug)]
pub struct GeminiImageProvider {
    client: Client,
    api_key: String,
    endpoint: String,
    default_model: String,
    model_definitions: HashMap<String, ModelDefinition>,
}

impl GeminiImageProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(IMAGE_GEN_LONG_POLL_TIMEOUT)
            .connect_timeout(HTTP_STREAM_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|e| {
                error!(error = %e, "Failed to build HTTP client for GeminiImageProvider, using default");
                Client::new()
            });

        Self {
            client,
            api_key,
            endpoint: "https://generativelanguage.googleapis.com/v1beta".to_owned(),
            default_model: "gemini-2.5-flash-image".to_owned(),
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

    async fn fetch_response(&self, model: &str, body: &GeminiRequest) -> Result<GeminiResponse> {
        let url = format!("{}/models/{}:generateContent", self.endpoint, model);

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|e| AiError::ProviderError {
                provider: self.name().to_owned(),
                message: format!("HTTP request failed: {e}"),
            })?;

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

        response.json().await.map_err(|e| AiError::ProviderError {
            provider: self.name().to_owned(),
            message: format!("Failed to parse response: {e}"),
        })
    }
}

#[async_trait]
impl ImageProvider for GeminiImageProvider {
    fn name(&self) -> &'static str {
        "gemini-image"
    }

    fn capabilities(&self) -> ImageProviderCapabilities {
        ImageProviderCapabilities {
            supported_resolutions: vec![
                ImageResolution::OneK,
                ImageResolution::TwoK,
                ImageResolution::FourK,
            ],
            supported_aspect_ratios: vec![
                AspectRatio::Square,
                AspectRatio::Landscape169,
                AspectRatio::Portrait916,
                AspectRatio::Landscape43,
                AspectRatio::Portrait34,
                AspectRatio::UltraWide,
            ],
            supports_batch: true,
            supports_image_editing: true,
            supports_search_grounding: true,
            max_prompt_length: 8000,
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

        let gemini_request = build_image_request(request, model, &self.model_definitions);

        let gemini_response = self.fetch_response(model, &gemini_request).await?;

        let (image_data, mime_type) = extract_image_from_response(&gemini_response)?;

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(ImageGenerationResponse::new(NewImageGenerationResponse {
            provider: self.name().to_owned(),
            model: model.to_owned(),
            image_data,
            mime_type,
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
