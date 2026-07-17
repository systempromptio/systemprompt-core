//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiToolCallId;

use super::captures::{CapturedToolUse, CapturedUsage};
use super::protocol::canonical::CanonicalContent;
use super::protocol::canonical_response::CanonicalResponse;

pub fn extract_from_canonical(
    response: &CanonicalResponse,
) -> (CapturedUsage, Vec<CapturedToolUse>) {
    let usage = CapturedUsage {
        input_tokens: response.usage.input_tokens,
        output_tokens: response.usage.output_tokens,
        cache_read_tokens: response.usage.cache_read_tokens,
        cache_creation_tokens: response.usage.cache_creation_tokens,
    };
    let mut tool_calls = Vec::new();
    for part in &response.content {
        if let CanonicalContent::ToolUse {
            id, name, input, ..
        } = part
        {
            tool_calls.push(CapturedToolUse {
                ai_tool_call_id: AiToolCallId::new(id.clone()),
                tool_name: name.clone(),
                tool_input: serde_json::to_string(input).unwrap_or_else(|e| {
                    tracing::warn!(error = %e, tool = %name, "failed to serialise tool_input");
                    String::new()
                }),
            });
        }
    }
    (usage, tool_calls)
}

pub fn extract_assistant_text(response: &CanonicalResponse) -> Option<String> {
    let mut out = String::new();
    for part in &response.content {
        if let CanonicalContent::Text(t) = part {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    if out.is_empty() { None } else { Some(out) }
}
