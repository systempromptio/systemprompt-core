use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ImageResolution {
    #[default]
    #[serde(rename = "1K")]
    OneK,
    #[serde(rename = "2K")]
    TwoK,
    #[serde(rename = "4K")]
    FourK,
}

impl ImageResolution {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::OneK => "1K",
            Self::TwoK => "2K",
            Self::FourK => "4K",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AspectRatio {
    #[default]
    #[serde(rename = "1:1")]
    Square,
    #[serde(rename = "16:9")]
    Landscape169,
    #[serde(rename = "9:16")]
    Portrait916,
    #[serde(rename = "4:3")]
    Landscape43,
    #[serde(rename = "3:4")]
    Portrait34,
    #[serde(rename = "21:9")]
    UltraWide,
}

impl AspectRatio {
    pub const fn as_str(&self) -> &str {
        match self {
            Self::Square => "1:1",
            Self::Landscape169 => "16:9",
            Self::Portrait916 => "9:16",
            Self::Landscape43 => "4:3",
            Self::Portrait34 => "3:4",
            Self::UltraWide => "21:9",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub model: Option<String>,
    #[serde(default)]
    pub resolution: ImageResolution,
    #[serde(default)]
    pub aspect_ratio: AspectRatio,
    #[serde(default)]
    pub reference_images: Vec<ReferenceImage>,
    #[serde(default)]
    pub enable_search_grounding: bool,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub mcp_execution_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceImage {
    pub data: String,
    pub mime_type: String,
    pub description: Option<String>,
}

#[derive(Debug)]
pub struct NewImageGenerationResponse {
    pub provider: String,
    pub model: String,
    pub image_data: String,
    pub mime_type: String,
    pub resolution: ImageResolution,
    pub aspect_ratio: AspectRatio,
    pub generation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationResponse {
    pub id: String,
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub image_data: String,
    pub mime_type: String,
    pub file_path: Option<String>,
    pub public_url: Option<String>,
    pub file_size_bytes: Option<usize>,
    pub resolution: ImageResolution,
    pub aspect_ratio: AspectRatio,
    pub generation_time_ms: u64,
    pub cost_estimate: Option<f32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ImageGenerationResponse {
    pub fn new(params: NewImageGenerationResponse) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            provider: params.provider,
            model: params.model,
            image_data: params.image_data,
            mime_type: params.mime_type,
            file_path: None,
            public_url: None,
            file_size_bytes: None,
            resolution: params.resolution,
            aspect_ratio: params.aspect_ratio,
            generation_time_ms: params.generation_time_ms,
            cost_estimate: None,
            created_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImageRecord {
    pub uuid: String,
    pub request_id: String,
    pub prompt: String,
    pub model: String,
    pub provider: String,
    pub file_path: String,
    pub public_url: String,
    pub file_size_bytes: Option<i32>,
    pub mime_type: String,
    pub resolution: Option<String>,
    pub aspect_ratio: Option<String>,
    pub generation_time_ms: Option<i32>,
    pub cost_estimate: Option<f32>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub trace_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}
