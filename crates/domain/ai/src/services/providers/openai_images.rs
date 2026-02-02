use crate::error::{AiError, Result};
use crate::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageGenerationResponse, ImageResolution,
    NewImageGenerationResponse,
};
use crate::services::providers::image_provider_trait::{ImageProvider, ImageProviderCapabilities};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug)]
pub struct OpenAiImageProvider {
    client: Client,
    api_key: String,
    endpoint: String,
    default_model: String,
}

impl OpenAiImageProvider {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            api_key,
            endpoint: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-image-1".to_string(),
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

    const fn map_size(aspect_ratio: &AspectRatio) -> &'static str {
        match aspect_ratio {
            AspectRatio::Square => "1024x1024",
            AspectRatio::Portrait916 | AspectRatio::Portrait34 => "1024x1792",
            _ => "1792x1024",
        }
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
            cost_per_image_cents: 4.0,
        }
    }

    fn supported_models(&self) -> Vec<String> {
        vec![
            "gpt-image-1".to_string(),
            "gpt-image-1-mini".to_string(),
            "dall-e-3".to_string(),
            "dall-e-2".to_string(),
        ]
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    async fn generate_image(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse> {
        let start = Instant::now();

        if request.prompt.len() > self.capabilities().max_prompt_length {
            return Err(AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!(
                    "Prompt length {} exceeds maximum {}",
                    request.prompt.len(),
                    self.capabilities().max_prompt_length
                ),
            });
        }

        if !self.supports_resolution(&request.resolution) {
            return Err(AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!("Resolution {} not supported", request.resolution.as_str()),
            });
        }

        if !self.supports_aspect_ratio(&request.aspect_ratio) {
            return Err(AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!(
                    "Aspect ratio {} not supported",
                    request.aspect_ratio.as_str()
                ),
            });
        }

        let model = request
            .model
            .as_deref()
            .unwrap_or_else(|| self.default_model());

        if !self.supports_model(model) {
            return Err(AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!("Model {model} not supported"),
            });
        }

        let dalle_request = DalleRequest {
            model: model.to_string(),
            prompt: request.prompt.clone(),
            size: Self::map_size(&request.aspect_ratio).to_string(),
            quality: "standard".to_string(),
            n: 1,
            response_format: "b64_json".to_string(),
        };

        let url = format!("{}/images/generations", self.endpoint);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&dalle_request)
            .send()
            .await
            .map_err(|e| AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!("HTTP request failed: {e}"),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|e| format!("<error reading response: {}>", e));
            return Err(AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!("API returned status {status}: {error_body}"),
            });
        }

        let dalle_response: DalleResponse =
            response.json().await.map_err(|e| AiError::ProviderError {
                provider: self.name().to_string(),
                message: format!("Failed to parse response: {e}"),
            })?;

        let image_data = dalle_response
            .data
            .first()
            .and_then(|d| d.b64_json.clone())
            .ok_or_else(|| AiError::ProviderError {
                provider: self.name().to_string(),
                message: "No image data in response".to_string(),
            })?;

        let generation_time_ms = start.elapsed().as_millis() as u64;

        Ok(ImageGenerationResponse::new(NewImageGenerationResponse {
            provider: self.name().to_string(),
            model: model.to_string(),
            image_data,
            mime_type: "image/png".to_string(),
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
