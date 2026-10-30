//! Turning the gateway's buffered response back into a completion.
//!
//! [`super::adapter::CanonicalInbound`] renders every buffered answer in the
//! Anthropic Messages shape, whichever wire the upstream spoke, so one parser
//! reads it back. A non-2xx answer is the upstream's own refusal and surfaces
//! with its status and body. Structured output is read from the forced
//! structured-output tool call when the upstream wire models it that way, and
//! otherwise from the response text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::response::Response;
use serde_json::Value;
use systemprompt_identifiers::AiRequestId;

use super::error::{DecodeError, GatewayClientError};
use crate::protocol::canonical::{CanonicalContent, CanonicalResponse, ResponseFormat};

const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// One completed in-process model call.
#[derive(Debug, Clone)]
pub struct GatewayCompletion {
    pub text: String,
    // JSON: structured output shaped by the caller's JSON Schema.
    pub structured: Option<Value>,
    pub usage: CompletionUsage,
    pub provider: String,
    pub model: String,
    pub ai_request_id: AiRequestId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletionUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug)]
pub(super) struct DecodeInputs<'a> {
    pub ai_request_id: AiRequestId,
    pub provider: String,
    pub fallback_model: &'a str,
    pub format: Option<ResponseFormat>,
}

pub(super) async fn decode(
    response: Response<Body>,
    inputs: DecodeInputs<'_>,
) -> Result<GatewayCompletion, GatewayClientError> {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BYTES)
        .await
        .map_err(|e| GatewayClientError::Decode(DecodeError::Body(e)))?;
    if !status.is_success() {
        return Err(GatewayClientError::Upstream {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        });
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| GatewayClientError::Decode(DecodeError::Json(e)))?;
    let canonical = systemprompt_wire::anthropic::parse_response(&value, inputs.fallback_model)
        .map_err(|e| GatewayClientError::Decode(DecodeError::Wire(e)))?;
    let text = response_text(&canonical);
    let structured = inputs
        .format
        .as_ref()
        .map(|format| structured_output(&canonical, &text, format))
        .transpose()
        .map_err(|e| GatewayClientError::Decode(DecodeError::Structured(e)))?;
    Ok(GatewayCompletion {
        text,
        structured,
        usage: CompletionUsage {
            input_tokens: canonical.usage.input_tokens,
            output_tokens: canonical.usage.output_tokens,
        },
        provider: inputs.provider,
        model: canonical.model,
        ai_request_id: inputs.ai_request_id,
    })
}

fn response_text(response: &CanonicalResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|part| match part {
            CanonicalContent::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn structured_output(
    response: &CanonicalResponse,
    text: &str,
    format: &ResponseFormat,
) -> Result<Value, serde_json::Error> {
    if let ResponseFormat::JsonSchema { name, .. } = format {
        let forced = response.content.iter().find_map(|part| match part {
            CanonicalContent::ToolUse {
                name: tool, input, ..
            } if tool == name => Some(input.clone()),
            _ => None,
        });
        if let Some(input) = forced {
            return Ok(input);
        }
    }
    serde_json::from_str(text)
}
