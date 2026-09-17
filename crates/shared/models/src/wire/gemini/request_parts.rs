//! Canonical content blocks rendered as Gemini `parts`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use serde_json::{Value, json};

use super::wire::{GeminiFunctionCall, GeminiFunctionResponse, GeminiInlineData, GeminiPart};
use crate::wire::canonical::{CanonicalContent, ImageSource};

pub(super) fn content_to_part(
    part: &CanonicalContent,
    call_names: &HashMap<&str, &str>,
) -> GeminiPart {
    match part {
        CanonicalContent::Text { text, .. } => plain_text_part(text.clone()),
        CanonicalContent::Image { source, .. } => image_part(source),
        CanonicalContent::ToolUse {
            name,
            input,
            signature,
            ..
        } => GeminiPart::FunctionCall {
            function_call: GeminiFunctionCall {
                name: name.clone(),
                args: input.clone(),
            },
            thought_signature: signature.clone(),
        },
        CanonicalContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            structured_content,
            ..
        } => tool_result_part(
            call_names
                .get(tool_use_id.as_str())
                .copied()
                .unwrap_or(tool_use_id),
            content,
            *is_error,
            structured_content.as_ref(),
        ),
        CanonicalContent::Thinking {
            text, signature, ..
        } => GeminiPart::Text {
            text: text.clone(),
            thought: Some(true),
            thought_signature: signature.clone(),
        },
    }
}

pub(super) const fn plain_text_part(text: String) -> GeminiPart {
    GeminiPart::Text {
        text,
        thought: None,
        thought_signature: None,
    }
}

fn image_part(src: &ImageSource) -> GeminiPart {
    match src {
        ImageSource::Base64 {
            media_type, data, ..
        } => GeminiPart::InlineData {
            inline_data: GeminiInlineData {
                mime_type: media_type.clone(),
                data: data.clone(),
            },
        },
        // Why: Gemini image parts require inline base64 data or a Files API handle,
        // not an arbitrary image URL.
        ImageSource::Url { url, .. } => {
            tracing::warn!(
                url = %url,
                "Gemini accepts only inline base64 image data; the image URL was \
                 downgraded to a plain text part and will not be seen as an image"
            );
            plain_text_part(url.clone())
        },
    }
}

fn tool_result_part(
    function_name: &str,
    content: &[CanonicalContent],
    is_error: bool,
    structured_content: Option<&Value>,
) -> GeminiPart {
    let response = if is_error {
        json!({ "error": flatten_text(content) })
    } else if let Some(sc) = structured_content {
        json!({ "result": sc })
    } else {
        json!({ "result": flatten_text(content) })
    };
    GeminiPart::FunctionResponse {
        function_response: GeminiFunctionResponse {
            name: function_name.to_owned(),
            response,
        },
    }
}

fn flatten_text(parts: &[CanonicalContent]) -> String {
    let mut out = String::new();
    for p in parts {
        if let CanonicalContent::Text { text: t, .. } = p {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}
