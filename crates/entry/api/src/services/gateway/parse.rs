//! Gateway request parsing helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiToolCallId;

use super::captures::CapturedToolUse;
use super::protocol::canonical::CanonicalContent;
use super::protocol::canonical_response::{CanonicalResponse, CanonicalUsage};

pub fn extract_from_canonical(
    response: &CanonicalResponse,
) -> (CanonicalUsage, Vec<CapturedToolUse>) {
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
    (response.usage, tool_calls)
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
