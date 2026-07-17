//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone, Copy, Default)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a token count; the `_tokens` suffix is the domain vocabulary shared \
              with the provider usage wire formats"
)]
pub struct CapturedUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_creation_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct CapturedToolUse {
    pub ai_tool_call_id: AiToolCallId,
    pub tool_name: String,
    pub tool_input: String,
}
