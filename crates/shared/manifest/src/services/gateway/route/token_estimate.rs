//! Conservative text-size estimation for conditional gateway routing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_wire::canonical::{CanonicalContent, CanonicalRequest};

pub(super) fn estimate_input_tokens(request: &CanonicalRequest) -> u32 {
    let mut chars = request
        .system
        .iter()
        .map(|block| block.text.len())
        .sum::<usize>();
    for message in &request.messages {
        for part in &message.content {
            accumulate_text_len(part, &mut chars);
        }
    }
    u32::try_from(chars / 4 + 1).unwrap_or(u32::MAX)
}

fn accumulate_text_len(part: &CanonicalContent, acc: &mut usize) {
    match part {
        CanonicalContent::AnthropicToolBlock { block, .. } => *acc += block.to_string().len(),
        CanonicalContent::Text { text, .. } | CanonicalContent::Thinking { text, .. } => {
            *acc += text.len();
        },
        CanonicalContent::ToolResult { content, .. } => {
            for inner in content {
                accumulate_text_len(inner, acc);
            }
        },
        CanonicalContent::ToolUse { .. } | CanonicalContent::Image { .. } => {},
    }
}
