#[derive(Debug, Clone, Copy, Default)]
pub struct CapturedUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct CapturedToolUse {
    pub ai_tool_call_id: String,
    pub tool_name: String,
    pub tool_input: String,
}
