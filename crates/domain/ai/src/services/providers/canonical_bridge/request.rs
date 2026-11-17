//! Canonical request construction from agent generation parameters.
//!
//! Applies provider-specific sampling and reasoning settings. Vendor wire
//! serialization is implemented by `systemprompt_wire`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_json::json;
use systemprompt_identifiers::ModelId;
use systemprompt_wire::canonical::{
    CanonicalReasoning, CanonicalRequest, CanonicalSampling, CanonicalTool, CanonicalToolChoice,
    CanonicalTooling, ReasoningEffort, ResponseFormat, SearchConfig, ThinkingConfig,
};
use systemprompt_wire::inspect::ForwardedSurface;

use crate::models::ai::{
    AiMessage, ReasoningEffort as AgentReasoningEffort, ReasoningParams,
    ResponseFormat as AgentResponseFormat, SamplingParams,
};
use crate::models::tools::McpTool;

use super::messages::messages_to_canonical;

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
    reasoning: Option<ReasoningParams>,
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
            reasoning: None,
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
    pub const fn with_reasoning(mut self, reasoning: Option<ReasoningParams>) -> Self {
        self.reasoning = reasoning;
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
            model: ModelId::new(self.model),
            cache_control: None,
            system,
            messages,
            max_tokens: self.max_output_tokens,
            stream: self.stream,
            metadata: None,
            response_format: self.response_format,
            forwarded_surface: ForwardedSurface::default(),
            sampling: CanonicalSampling {
                temperature,
                top_p: self.sampling.and_then(|s| s.top_p),
                top_k: self.sampling.and_then(|s| s.top_k),
                stop_sequences: self
                    .sampling
                    .and_then(|s| s.stop_sequences.clone())
                    .unwrap_or_default(),
                presence_penalty: self.sampling.and_then(|s| s.presence_penalty),
                frequency_penalty: self.sampling.and_then(|s| s.frequency_penalty),
            },
            tooling: CanonicalTooling {
                tools: self.tools,
                tool_choice: self.tool_choice,
                search: self.search,
                code_execution: self.code_execution,
            },
            reasoning: CanonicalReasoning {
                thinking: self
                    .reasoning
                    .and_then(|r| r.thinking_budget_tokens)
                    .map(|budget| ThinkingConfig {
                        enabled: true,
                        budget_tokens: Some(budget),
                    })
                    .or_else(|| auto_thinking(self.provider, self.model)),
                reasoning_effort: self
                    .reasoning
                    .and_then(|r| r.effort)
                    .map(canonical_effort)
                    .or_else(|| auto_reasoning(self.provider, self.model)),
            },
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

const fn canonical_effort(effort: AgentReasoningEffort) -> ReasoningEffort {
    match effort {
        AgentReasoningEffort::Low => ReasoningEffort::Low,
        AgentReasoningEffort::Medium => ReasoningEffort::Medium,
        AgentReasoningEffort::High => ReasoningEffort::High,
    }
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
            anthropic_definition: None,
            name: t.name,
            description: t.description,
            input_schema: t
                .input_schema
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} })),
            cache_control: None,
        })
        .collect()
}
