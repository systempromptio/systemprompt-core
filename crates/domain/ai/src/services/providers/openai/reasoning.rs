use crate::models::providers::openai::OpenAiReasoningEffort;

pub fn build_reasoning_config(model: &str) -> Option<OpenAiReasoningEffort> {
    if is_reasoning_model(model) {
        Some(OpenAiReasoningEffort::Medium)
    } else {
        None
    }
}

pub fn is_reasoning_model(model: &str) -> bool {
    model.starts_with("o1") || model.starts_with("o3")
}
