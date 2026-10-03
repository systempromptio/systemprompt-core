//! Capture buffers for gateway audit payloads.
//!
//! Usage is not captured separately: `CanonicalUsage` already carries the six
//! counts with their convention attached, and a parallel struct only invited a
//! lossy round trip on the way to billing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone)]
pub struct CapturedToolUse {
    pub ai_tool_call_id: AiToolCallId,
    pub tool_name: String,
    pub tool_input: String,
}
