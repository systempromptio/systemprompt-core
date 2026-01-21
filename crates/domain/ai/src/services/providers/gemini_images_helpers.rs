use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageResolution};
use crate::models::providers::gemini::{
    GeminiContent, GeminiGenerationConfig, GeminiImageConfig, GeminiInlineData, GeminiPart,
    GeminiRequest, GeminiResponse, GeminiTool, GoogleSearch,
};
use std::collections::HashMap;
use systemprompt_models::services::ModelDefinition;

pub fn map_resolution_to_gemini_size(resolution: &ImageResolution) -> String {
    match resolution {
        ImageResolution::OneK => "1K".to_string(),
        ImageResolution::TwoK => "2K".to_string(),
        ImageResolution::FourK => "4K".to_string(),
    }
}

pub fn model_supports_image_size(
    model_definitions: &HashMap<String, ModelDefinition>,
    model: &str,
) -> bool {
    model_definitions
        .get(model)
        .is_some_and(|def| def.capabilities.image_resolution_config)
}

pub fn build_image_request(
    request: &ImageGenerationRequest,
    model: &str,
    model_definitions: &HashMap<String, ModelDefinition>,
) -> GeminiRequest {
    let mut parts = vec![GeminiPart::Text {
        text: request.prompt.clone(),
    }];

    for ref_image in &request.reference_images {
        parts.push(GeminiPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: ref_image.mime_type.clone(),
                data: ref_image.data.clone(),
            },
        });
        if let Some(desc) = &ref_image.description {
            parts.push(GeminiPart::Text { text: desc.clone() });
        }
    }

    let contents = vec![GeminiContent {
        role: "user".to_string(),
        parts,
    }];

    let image_size = model_supports_image_size(model_definitions, model)
        .then(|| map_resolution_to_gemini_size(&request.resolution));

    let generation_config = GeminiGenerationConfig {
        temperature: None,
        top_p: None,
        top_k: None,
        max_output_tokens: None,
        stop_sequences: None,
        response_mime_type: None,
        response_schema: None,
        response_modalities: Some(vec!["IMAGE".to_string()]),
        image_config: Some(GeminiImageConfig {
            aspect_ratio: request.aspect_ratio.as_str().to_string(),
            image_size,
        }),
        thinking_config: None,
    };

    let tools = if request.enable_search_grounding {
        Some(vec![GeminiTool {
            function_declarations: None,
            google_search: Some(GoogleSearch {}),
            url_context: None,
            code_execution: None,
        }])
    } else {
        None
    };

    GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools,
        tool_config: None,
    }
}

pub fn extract_image_from_response(response: &GeminiResponse) -> Result<(String, String)> {
    let candidate =
        response
            .candidates
            .first()
            .ok_or_else(|| AiError::EmptyProviderResponse {
                provider: "gemini-image".to_string(),
            })?;

    let content = candidate
        .content
        .as_ref()
        .ok_or_else(|| AiError::ProviderError {
            provider: "gemini-image".to_string(),
            message: "Image generation returned empty response - this may indicate the prompt \
                      was rejected by content safety filters, API quota exceeded, or a \
                      transient service error. Please inform the user that image generation \
                      failed and the content was created without an image."
                .to_string(),
        })?;

    for part in &content.parts {
        if let GeminiPart::InlineData { inline_data } = part {
            return Ok((inline_data.data.clone(), inline_data.mime_type.clone()));
        }
    }

    Err(AiError::ProviderError {
        provider: "gemini-image".to_string(),
        message: "No image data found in response".to_string(),
    })
}
