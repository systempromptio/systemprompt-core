//! Image metadata and provenance value types.

use serde::{Deserialize, Serialize};

/// Image-specific metadata persisted alongside generated files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageMetadata {
    /// Image width in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,

    /// Image height in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    /// Accessibility text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,

    /// Long-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Generation provenance, if the image was AI-produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<ImageGenerationInfo>,
}

/// Provenance details for an AI-generated image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationInfo {
    /// Prompt used to generate the image.
    pub prompt: String,
    /// Model identifier.
    pub model: String,
    /// Provider name (`openai`, `gemini`, ...).
    pub provider: String,

    /// Resolution string (`1024x1024`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,

    /// Aspect ratio string (`16:9`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,

    /// Generation latency in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_time_ms: Option<i32>,

    /// Estimated cost of the generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_estimate: Option<f32>,

    /// Provider-side request id for traceability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ImageMetadata {
    /// Construct empty image metadata.
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            alt_text: None,
            description: None,
            generation: None,
        }
    }

    /// Attach pixel dimensions.
    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// Attach accessibility alt-text.
    pub fn with_alt_text(mut self, alt: impl Into<String>) -> Self {
        self.alt_text = Some(alt.into());
        self
    }

    /// Attach a long-form description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Attach generation provenance.
    pub fn with_generation(mut self, generation: ImageGenerationInfo) -> Self {
        self.generation = Some(generation);
        self
    }
}

impl ImageGenerationInfo {
    /// Construct provenance metadata with the required fields.
    pub fn new(
        prompt: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        Self {
            prompt: prompt.into(),
            model: model.into(),
            provider: provider.into(),
            resolution: None,
            aspect_ratio: None,
            generation_time_ms: None,
            cost_estimate: None,
            request_id: None,
        }
    }

    /// Attach a resolution string.
    pub fn with_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }

    /// Attach an aspect ratio string.
    pub fn with_aspect_ratio(mut self, aspect_ratio: impl Into<String>) -> Self {
        self.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    /// Attach a generation latency.
    pub const fn with_generation_time(mut self, time_ms: i32) -> Self {
        self.generation_time_ms = Some(time_ms);
        self
    }

    /// Attach a cost estimate.
    pub const fn with_cost_estimate(mut self, cost: f32) -> Self {
        self.cost_estimate = Some(cost);
        self
    }

    /// Attach a provider-side request id.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}
