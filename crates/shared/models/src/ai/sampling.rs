use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPreferences {
    pub hints: Vec<ModelHint>,
    pub cost_priority: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelHint {
    ModelId(String),
    Category(String),
    Provider(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SamplingParams {
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider: String,
    pub model: String,
    pub max_output_tokens: u32,
}

impl ProviderConfig {
    pub fn new(
        provider: impl Into<String>,
        model: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            max_output_tokens,
        }
    }
}
