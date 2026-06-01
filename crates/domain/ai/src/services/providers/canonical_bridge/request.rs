//! Builds a [`CanonicalRequest`] from the agent's generation parameters.
//!
//! This owns the per-provider sampling and reasoning *policy* that the deleted
//! per-provider request builders used to carry: Anthropic extended-thinking for
//! the claude-3-5 family, `OpenAI` reasoning effort for the o1/o3 families, and
//! the `OpenAI` streaming temperature default. Vendor wire rendering itself
//! lives in [`systemprompt_models::wire`]; this module only assembles the
//! canonical request the codec consumes.

use serde_json::json;
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalTool, CanonicalToolChoice,
    ImageSource, ReasoningEffort, ResponseFormat, Role, SearchConfig, ThinkingConfig,
};

use crate::models::ai::{
    AiContentPart, AiMessage, MessageRole, ResponseFormat as AgentResponseFormat, SamplingParams,
};
use crate::models::tools::McpTool;

const DEFAULT_SCHEMA_NAME: &str = "structured_output";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeProvider {
    Anthropic,
    OpenAi,
    Gemini,
}

const ANTHROPIC_THINKING_BUDGET: u32 = 10240;
const OPENAI_STREAM_DEFAULT_TEMPERATURE: f32 = 0.8;

#[derive(Debug)]
pub struct CanonicalBuild<'a> {
    provider: BridgeProvider,
    messages: &'a [AiMessage],
    model: &'a str,
    max_output_tokens: u32,
    sampling: Option<&'a SamplingParams>,
    tools: Vec<CanonicalTool>,
    tool_choice: Option<CanonicalToolChoice>,
    response_format: Option<ResponseFormat>,
    search: Option<SearchConfig>,
    code_execution: bool,
    stream: bool,
}

impl<'a> CanonicalBuild<'a> {
    pub const fn new(
        provider: BridgeProvider,
        messages: &'a [AiMessage],
        model: &'a str,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            provider,
            messages,
            model,
            max_output_tokens,
            sampling: None,
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            search: None,
            code_execution: false,
            stream: false,
        }
    }

    #[must_use]
    pub const fn with_sampling(mut self, sampling: Option<&'a SamplingParams>) -> Self {
        self.sampling = sampling;
        self
    }

    #[must_use]
    pub fn with_tools(mut self, tools: Vec<CanonicalTool>) -> Self {
        self.tools = tools;
        self
    }

    #[must_use]
    pub fn with_tool_choice(mut self, tool_choice: Option<CanonicalToolChoice>) -> Self {
        self.tool_choice = tool_choice;
        self
    }

    #[must_use]
    pub fn with_response_format(mut self, response_format: Option<ResponseFormat>) -> Self {
        self.response_format = response_format;
        self
    }

    #[must_use]
    pub fn with_search(mut self, search: Option<SearchConfig>) -> Self {
        self.search = search;
        self
    }

    #[must_use]
    pub const fn with_code_execution(mut self, code_execution: bool) -> Self {
        self.code_execution = code_execution;
        self
    }

    #[must_use]
    pub const fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    #[must_use]
    pub fn into_request(self) -> CanonicalRequest {
        let (system, messages) = messages_to_canonical(self.messages);
        let mut temperature = self.sampling.and_then(|s| s.temperature);
        if temperature.is_none() && self.stream && self.provider == BridgeProvider::OpenAi {
            temperature = Some(OPENAI_STREAM_DEFAULT_TEMPERATURE);
        }
        CanonicalRequest {
            model: self.model.to_owned(),
            system,
            messages,
            max_tokens: self.max_output_tokens,
            temperature,
            top_p: self.sampling.and_then(|s| s.top_p),
            top_k: self.sampling.and_then(|s| s.top_k),
            stop_sequences: self
                .sampling
                .and_then(|s| s.stop_sequences.clone())
                .unwrap_or_default(),
            tools: self.tools,
            tool_choice: self.tool_choice,
            stream: self.stream,
            thinking: auto_thinking(self.provider, self.model),
            metadata: None,
            response_format: self.response_format,
            reasoning_effort: auto_reasoning(self.provider, self.model),
            search: self.search,
            code_execution: self.code_execution,
            presence_penalty: self.sampling.and_then(|s| s.presence_penalty),
            frequency_penalty: self.sampling.and_then(|s| s.frequency_penalty),
        }
    }
}

fn auto_thinking(provider: BridgeProvider, model: &str) -> Option<ThinkingConfig> {
    if provider == BridgeProvider::Anthropic
        && (model.contains("claude-3-5") || model.contains("claude-3.5"))
    {
        return Some(ThinkingConfig {
            enabled: true,
            budget_tokens: Some(ANTHROPIC_THINKING_BUDGET),
        });
    }
    None
}

fn auto_reasoning(provider: BridgeProvider, model: &str) -> Option<ReasoningEffort> {
    if provider == BridgeProvider::OpenAi && (model.starts_with("o1") || model.starts_with("o3")) {
        return Some(ReasoningEffort::Medium);
    }
    None
}

#[must_use]
pub fn agent_response_format(format: &AgentResponseFormat) -> Option<ResponseFormat> {
    match format {
        AgentResponseFormat::Text => None,
        AgentResponseFormat::JsonObject => Some(ResponseFormat::JsonObject),
        AgentResponseFormat::JsonSchema {
            schema,
            name,
            strict,
        } => Some(ResponseFormat::JsonSchema {
            name: name
                .clone()
                .unwrap_or_else(|| DEFAULT_SCHEMA_NAME.to_owned()),
            schema: schema.clone(),
            strict: strict.unwrap_or(true),
        }),
    }
}

pub fn tools_to_canonical(tools: Vec<McpTool>) -> Vec<CanonicalTool> {
    tools
        .into_iter()
        .map(|t| CanonicalTool {
            name: t.name,
            description: t.description,
            input_schema: t
                .input_schema
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
        })
        .collect()
}

fn messages_to_canonical(messages: &[AiMessage]) -> (Option<String>, Vec<CanonicalMessage>) {
    let mut system: Option<String> = None;
    let mut out: Vec<CanonicalMessage> = Vec::new();
    for message in messages {
        match message.role {
            MessageRole::System => match &mut system {
                Some(existing) => {
                    existing.push('\n');
                    existing.push_str(&message.content);
                },
                None => system = Some(message.content.clone()),
            },
            MessageRole::User | MessageRole::Assistant => {
                let role = if matches!(message.role, MessageRole::Assistant) {
                    Role::Assistant
                } else {
                    Role::User
                };
                let content = message_content(message);
                if !content.is_empty() {
                    out.push(CanonicalMessage { role, content });
                }
            },
        }
    }
    (system, out)
}

fn message_content(message: &AiMessage) -> Vec<CanonicalContent> {
    let mut content: Vec<CanonicalContent> = Vec::new();
    if !message.content.is_empty() {
        content.push(CanonicalContent::Text(message.content.clone()));
    }
    for part in &message.parts {
        match part {
            AiContentPart::Text { text } => content.push(CanonicalContent::Text(text.clone())),
            AiContentPart::Image { mime_type, data } => {
                content.push(CanonicalContent::Image(ImageSource::Base64 {
                    media_type: mime_type.clone(),
                    data: data.clone(),
                    detail: None,
                }));
            },
            AiContentPart::Audio { .. } | AiContentPart::Video { .. } => {},
        }
    }
    content
}
