use crate::models::ai::{AiMessage, MessageRole, SamplingParams};
use crate::models::tools::{CallToolResult, ToolCall};
use crate::services::providers::{AiProvider, GenerationParams, ToolResultsParams};
use crate::services::tooled::ToolResultFormatter;
use tracing::{info, warn};

#[derive(Debug)]
pub enum FallbackReason {
    EmptyContent,
    SynthesisFailed(String),
}

#[derive(Debug, Copy, Clone)]
pub struct FallbackGenerator;

impl FallbackGenerator {
    pub const fn new() -> Self {
        Self
    }

    pub fn generate(
        tool_calls: &[ToolCall],
        tool_results: &[CallToolResult],
        reason: FallbackReason,
    ) -> String {
        let summary = ToolResultFormatter::format_fallback_summary(tool_calls, tool_results);

        match reason {
            FallbackReason::EmptyContent => {
                format!("Tool execution completed:\n\n{summary}")
            },
            FallbackReason::SynthesisFailed(error) => {
                format!("Tool execution completed:\n\n{summary}\n\n(Synthesis error: {error})")
            },
        }
    }
}

impl Default for FallbackGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct SynthesisPromptBuilder;

impl SynthesisPromptBuilder {
    pub fn build_guidance_message(
        tool_calls: &[ToolCall],
        tool_results: &[CallToolResult],
    ) -> AiMessage {
        let tool_summary = ToolResultFormatter::format_for_display(tool_calls, tool_results);

        AiMessage {
            role: MessageRole::User,
            content: format!(
                "The following tools were just executed:\n\n{tool_summary}\n\nBased on these tool \
                 execution results, provide a clear, natural language response to the user. Focus \
                 on:\n- What the results mean for the user\n- How they answer the user's \
                 question\n- Any important insights from the data\n\nBe concise but informative. \
                 Do not repeat the raw tool data - synthesize it into a helpful response."
            ),
            parts: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum SynthesisResult {
    Success(String),
    NeedsFallback { reason: FallbackReason },
}

pub struct SynthesisParams<'a> {
    pub provider: &'a dyn AiProvider,
    pub original_messages: &'a [AiMessage],
    pub tool_calls: &'a [ToolCall],
    pub tool_results: &'a [CallToolResult],
    pub sampling: Option<&'a SamplingParams>,
    pub max_output_tokens: u32,
    pub model: &'a str,
}

impl std::fmt::Debug for SynthesisParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SynthesisParams")
            .field("provider", &"<dyn AiProvider>")
            .field("original_messages", &self.original_messages)
            .field("tool_calls", &self.tool_calls)
            .field("tool_results", &self.tool_results)
            .field("sampling", &self.sampling)
            .field("max_output_tokens", &self.max_output_tokens)
            .field("model", &self.model)
            .finish()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ResponseSynthesizer;

impl ResponseSynthesizer {
    pub const fn new() -> Self {
        Self
    }

    pub async fn synthesize_or_fallback(&self, params: SynthesisParams<'_>) -> String {
        Self::log_synthesis_start(
            params.tool_calls.len(),
            params.tool_results.len(),
            params.model,
            params.original_messages.len(),
        );

        let synthesis_result = self.attempt_synthesis(&params).await;

        match synthesis_result {
            SynthesisResult::Success(content) => {
                Self::log_synthesis_success(&content);
                content
            },
            SynthesisResult::NeedsFallback { reason } => {
                Self::log_fallback_reason(&reason, params.tool_calls.len());
                FallbackGenerator::generate(params.tool_calls, params.tool_results, reason)
            },
        }
    }

    fn log_synthesis_start(tool_count: usize, result_count: usize, model: &str, msg_len: usize) {
        info!(
            tool_count = tool_count,
            result_count = result_count,
            model = model,
            conversation_length = msg_len,
            "Starting tool result synthesis"
        );
    }

    fn log_synthesis_success(content: &str) {
        let preview: String = content.chars().take(200).collect();
        info!(
            strategy = "ai_synthesis",
            content_length = content.len(),
            content_preview = %preview,
            "Synthesis succeeded"
        );
    }

    fn log_fallback_reason(reason: &FallbackReason, tool_count: usize) {
        let (reason_str, error_opt) = match reason {
            FallbackReason::EmptyContent => ("empty_content", None),
            FallbackReason::SynthesisFailed(e) => ("synthesis_error", Some(e.as_str())),
        };

        warn!(
            strategy = "fallback_generator",
            reason = reason_str,
            error = error_opt,
            tool_count = tool_count,
            "Synthesis failed, using fallback"
        );
    }

    async fn attempt_synthesis(&self, params: &SynthesisParams<'_>) -> SynthesisResult {
        let base = GenerationParams {
            messages: params.original_messages,
            model: params.model,
            max_output_tokens: params.max_output_tokens,
            sampling: params.sampling,
        };
        let tool_results_params =
            ToolResultsParams::new(base.clone(), params.tool_calls, params.tool_results);

        match params
            .provider
            .generate_with_tool_results(tool_results_params)
            .await
        {
            Ok(response) if !response.content.is_empty() => {
                return SynthesisResult::Success(response.content);
            },
            _ => {},
        }

        let mut enhanced_messages = params.original_messages.to_vec();
        enhanced_messages.push(SynthesisPromptBuilder::build_guidance_message(
            params.tool_calls,
            params.tool_results,
        ));

        let gen_params = GenerationParams {
            messages: &enhanced_messages,
            model: params.model,
            max_output_tokens: params.max_output_tokens,
            sampling: params.sampling,
        };

        match params.provider.generate(gen_params).await {
            Ok(response) if !response.content.is_empty() => {
                SynthesisResult::Success(response.content)
            },
            Ok(_) => SynthesisResult::NeedsFallback {
                reason: FallbackReason::EmptyContent,
            },
            Err(e) => SynthesisResult::NeedsFallback {
                reason: FallbackReason::SynthesisFailed(e.to_string()),
            },
        }
    }
}

impl Default for ResponseSynthesizer {
    fn default() -> Self {
        Self::new()
    }
}
