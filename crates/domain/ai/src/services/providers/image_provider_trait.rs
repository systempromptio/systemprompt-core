use crate::error::Result;
use crate::models::image_generation::{
    AspectRatio, ImageGenerationRequest, ImageGenerationResponse, ImageResolution,
};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ImageProviderCapabilities {
    pub supported_resolutions: Vec<ImageResolution>,
    pub supported_aspect_ratios: Vec<AspectRatio>,
    pub supports_batch: bool,
    pub supports_image_editing: bool,
    pub supports_search_grounding: bool,
    pub max_prompt_length: usize,
    pub cost_per_image_cents: f32,
}

#[async_trait]
pub trait ImageProvider: Send + Sync {
    fn name(&self) -> &str;

    fn capabilities(&self) -> ImageProviderCapabilities;

    fn supported_models(&self) -> Vec<String>;

    fn supports_model(&self, model: &str) -> bool {
        self.supported_models().iter().any(|m| m == model)
    }

    fn default_model(&self) -> &str;

    fn supports_resolution(&self, resolution: &ImageResolution) -> bool {
        self.capabilities()
            .supported_resolutions
            .contains(resolution)
    }

    fn supports_aspect_ratio(&self, aspect_ratio: &AspectRatio) -> bool {
        self.capabilities()
            .supported_aspect_ratios
            .contains(aspect_ratio)
    }

    async fn generate_image(
        &self,
        request: &ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse>;

    async fn generate_batch(
        &self,
        requests: &[ImageGenerationRequest],
    ) -> Result<Vec<ImageGenerationResponse>> {
        if !self.capabilities().supports_batch {
            return Err(crate::error::AiError::ProviderError {
                provider: self.name().to_string(),
                message: "Batch generation not supported by this provider".to_string(),
            });
        }

        let mut responses = Vec::new();
        for request in requests {
            responses.push(self.generate_image(request).await?);
        }
        Ok(responses)
    }
}

pub type BoxedImageProvider = Arc<dyn ImageProvider>;
