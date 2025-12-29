use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<ImageGenerationInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationInfo {
    pub prompt: String,
    pub model: String,
    pub provider: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_time_ms: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_estimate: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ImageMetadata {
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            alt_text: None,
            description: None,
            generation: None,
        }
    }

    pub const fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    pub fn with_alt_text(mut self, alt: impl Into<String>) -> Self {
        self.alt_text = Some(alt.into());
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_generation(mut self, gen: ImageGenerationInfo) -> Self {
        self.generation = Some(gen);
        self
    }
}

impl ImageGenerationInfo {
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

    pub fn with_resolution(mut self, resolution: impl Into<String>) -> Self {
        self.resolution = Some(resolution.into());
        self
    }

    pub fn with_aspect_ratio(mut self, aspect_ratio: impl Into<String>) -> Self {
        self.aspect_ratio = Some(aspect_ratio.into());
        self
    }

    pub const fn with_generation_time(mut self, time_ms: i32) -> Self {
        self.generation_time_ms = Some(time_ms);
        self
    }

    pub const fn with_cost_estimate(mut self, cost: f32) -> Self {
        self.cost_estimate = Some(cost);
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}
