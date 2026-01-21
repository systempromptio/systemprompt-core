use crate::models::ai::{AiMessage, SamplingParams};
use crate::models::providers::gemini::GeminiFunctionCallingMode;
use crate::models::tools::{CallToolResult, McpTool, ToolCall};

#[derive(Debug)]
pub struct ToolRequestParams<'a> {
    pub messages: &'a [AiMessage],
    pub tools: &'a [McpTool],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

#[derive(Debug)]
pub struct ToolRequestParamsBuilder<'a> {
    messages: &'a [AiMessage],
    tools: &'a [McpTool],
    sampling: Option<&'a SamplingParams>,
    max_output_tokens: u32,
    model: &'a str,
}

impl<'a> ToolRequestParamsBuilder<'a> {
    pub const fn new(
        messages: &'a [AiMessage],
        tools: &'a [McpTool],
        max_output_tokens: u32,
        model: &'a str,
    ) -> Self {
        Self {
            messages,
            tools,
            sampling: None,
            max_output_tokens,
            model,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub const fn build(self) -> ToolRequestParams<'a> {
        ToolRequestParams {
            messages: self.messages,
            tools: self.tools,
            sampling: self.sampling,
            max_output_tokens: self.max_output_tokens,
            model: self.model,
        }
    }
}

impl<'a> ToolRequestParams<'a> {
    pub const fn builder(
        messages: &'a [AiMessage],
        tools: &'a [McpTool],
        max_output_tokens: u32,
        model: &'a str,
    ) -> ToolRequestParamsBuilder<'a> {
        ToolRequestParamsBuilder::new(messages, tools, max_output_tokens, model)
    }
}

#[derive(Debug)]
pub struct ToolResultParams<'a> {
    pub conversation_history: &'a [AiMessage],
    pub tool_calls: &'a [ToolCall],
    pub tool_results: &'a [CallToolResult],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

#[derive(Debug)]
pub struct ToolResultParamsBuilder<'a> {
    conversation_history: &'a [AiMessage],
    tool_calls: &'a [ToolCall],
    tool_results: &'a [CallToolResult],
    sampling: Option<&'a SamplingParams>,
    max_output_tokens: u32,
    model: &'a str,
}

impl<'a> ToolResultParamsBuilder<'a> {
    pub const fn new(
        conversation_history: &'a [AiMessage],
        tool_calls: &'a [ToolCall],
        tool_results: &'a [CallToolResult],
        max_output_tokens: u32,
        model: &'a str,
    ) -> Self {
        Self {
            conversation_history,
            tool_calls,
            tool_results,
            sampling: None,
            max_output_tokens,
            model,
        }
    }

    pub const fn with_sampling(mut self, sampling: &'a SamplingParams) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub const fn build(self) -> ToolResultParams<'a> {
        ToolResultParams {
            conversation_history: self.conversation_history,
            tool_calls: self.tool_calls,
            tool_results: self.tool_results,
            sampling: self.sampling,
            max_output_tokens: self.max_output_tokens,
            model: self.model,
        }
    }
}

impl<'a> ToolResultParams<'a> {
    pub const fn builder(
        conversation_history: &'a [AiMessage],
        tool_calls: &'a [ToolCall],
        tool_results: &'a [CallToolResult],
        max_output_tokens: u32,
        model: &'a str,
    ) -> ToolResultParamsBuilder<'a> {
        ToolResultParamsBuilder::new(
            conversation_history,
            tool_calls,
            tool_results,
            max_output_tokens,
            model,
        )
    }
}

pub(super) struct ToolConfigParams<'a> {
    pub messages: &'a [AiMessage],
    pub tools: Vec<McpTool>,
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
    pub function_calling_mode: GeminiFunctionCallingMode,
    pub allowed_function_names: Option<Vec<String>>,
}

impl<'a> ToolConfigParams<'a> {
    pub fn new(base: &ToolRequestParams<'a>) -> Self {
        Self {
            messages: base.messages,
            tools: base.tools.to_vec(),
            sampling: base.sampling,
            max_output_tokens: base.max_output_tokens,
            model: base.model,
            function_calling_mode: GeminiFunctionCallingMode::Auto,
            allowed_function_names: None,
        }
    }
}
