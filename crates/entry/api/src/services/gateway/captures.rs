use systemprompt_identifiers::AiToolCallId;

#[derive(Debug, Clone, Copy, Default)]
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
