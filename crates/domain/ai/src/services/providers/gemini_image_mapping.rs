//! Maps Gemini image responses into the provider-neutral shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{AiError, Result};
use crate::models::image_generation::{ImageGenerationRequest, ImageResolution};
use std::collections::HashMap;
use systemprompt_manifest::services::ModelDefinition;
use systemprompt_wire::gemini::{
    GeminiContent, GeminiEmpty, GeminiGenerationConfig, GeminiImageConfig, GeminiInlineData,
    GeminiPart, GeminiRequest, GeminiResponse, GeminiTool,
};

pub(super) fn map_resolution_to_gemini_size(resolution: &ImageResolution) -> String {
    match resolution {
        ImageResolution::OneK => "1K".to_owned(),
        ImageResolution::TwoK => "2K".to_owned(),
        ImageResolution::FourK => "4K".to_owned(),
    }
}

pub(super) fn model_supports_image_size(
    model_definitions: &HashMap<String, ModelDefinition>,
    model: &str,
) -> bool {
    model_definitions
        .get(model)
        .is_some_and(|def| def.capabilities.image_resolution_config)
}

pub(super) fn build_image_request(
    request: &ImageGenerationRequest,
    model: &str,
    model_definitions: &HashMap<String, ModelDefinition>,
) -> GeminiRequest {
    let mut parts = vec![text_part(request.prompt.clone())];

    for ref_image in &request.reference_images {
        parts.push(GeminiPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: ref_image.mime_type.clone(),
                data: ref_image.data.clone(),
            },
        });
        if let Some(desc) = &ref_image.description {
            parts.push(text_part(desc.clone()));
        }
    }

    let contents = vec![GeminiContent {
        role: "user".to_owned(),
        parts,
    }];

    let image_size = model_supports_image_size(model_definitions, model)
        .then(|| map_resolution_to_gemini_size(&request.resolution));

    let generation_config = GeminiGenerationConfig {
        response_modalities: Some(vec!["IMAGE".to_owned()]),
        image_config: Some(GeminiImageConfig {
            aspect_ratio: request.aspect_ratio.as_str().to_owned(),
            image_size,
        }),
        ..GeminiGenerationConfig::default()
    };

    let tools = request.enable_search_grounding.then(|| {
        vec![GeminiTool::GoogleSearch {
            google_search: GeminiEmpty {},
        }]
    });

    GeminiRequest {
        contents,
        system_instruction: None,
        generation_config: Some(generation_config),
        tools,
        tool_config: None,
    }
}

const fn text_part(text: String) -> GeminiPart {
    GeminiPart::Text {
        text,
        thought: None,
        thought_signature: None,
    }
}

pub(super) fn extract_image_from_response(response: &GeminiResponse) -> Result<(String, String)> {
    let candidate = response
        .candidates
        .first()
        .ok_or_else(|| AiError::EmptyProviderResponse {
            provider: "gemini-image".to_owned(),
        })?;

    let content = candidate
        .content
        .as_ref()
        .ok_or_else(|| AiError::ProviderError {
            provider: "gemini-image".to_owned(),
            message: "Image generation returned empty response - this may indicate the prompt was \
                      rejected by content safety filters, API quota exceeded, or a transient \
                      service error. Please inform the user that image generation failed and the \
                      content was created without an image."
                .to_owned(),
        })?;

    for part in &content.parts {
        if let GeminiPart::InlineData { inline_data } = part {
            return Ok((inline_data.data.clone(), inline_data.mime_type.clone()));
        }
    }

    Err(AiError::ProviderError {
        provider: "gemini-image".to_owned(),
        message: "No image data found in response".to_owned(),
    })
}
