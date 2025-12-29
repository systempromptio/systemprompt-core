use anyhow::{anyhow, Result};
use std::time::{Duration, Instant};
use tracing::info;
use uuid::Uuid;

use crate::models::ai::{AiMessage, SamplingParams};
use crate::models::providers::gemini::{
    CodeExecution, GeminiCandidate, GeminiPart, GeminiRequest, GeminiResponse, GeminiTool,
};

use super::provider::GeminiProvider;
use super::{converters, request_builders};

#[derive(Debug, Clone)]
pub struct CodeExecutionResponse {
    pub generated_code: String,
    pub execution_output: String,
    pub success: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
}

#[derive(Debug, Default)]
struct CodeExtractionResult {
    generated_code: String,
    execution_output: String,
    execution_success: bool,
    execution_error: Option<String>,
}

fn build_code_execution_request(
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
) -> GeminiRequest {
    let contents = converters::convert_messages(messages);

    let tools = vec![GeminiTool {
        function_declarations: None,
        google_search: None,
        url_context: None,
        code_execution: Some(CodeExecution {}),
    }];

    let generation_config =
        request_builders::build_generation_config(sampling, max_output_tokens, None, None);

    GeminiRequest {
        contents,
        generation_config: Some(generation_config),
        safety_settings: None,
        tools: Some(tools),
        tool_config: None,
    }
}

async fn send_and_parse_request(
    provider: &GeminiProvider,
    request: &GeminiRequest,
    model: &str,
    request_id: Uuid,
) -> Result<GeminiResponse> {
    let response_text =
        request_builders::send_request(provider, request, model, "generateContent").await?;

    info!(
        request_id = %request_id,
        response_length = response_text.len(),
        "Received response"
    );

    request_builders::parse_response(&response_text)
}

fn extract_code_execution_result(candidate: &GeminiCandidate) -> Result<CodeExtractionResult> {
    let content = candidate.content.as_ref().ok_or_else(|| {
        let reason = candidate.finish_reason.as_deref().unwrap_or("UNKNOWN");
        anyhow!("Gemini returned no content for code execution. Finish reason: {reason}")
    })?;

    let mut result = CodeExtractionResult::default();

    for part in &content.parts {
        match part {
            GeminiPart::ExecutableCode { executable_code } => {
                result.generated_code.clone_from(&executable_code.code);
            },
            GeminiPart::CodeExecutionResult {
                code_execution_result,
            } => {
                result.execution_success = code_execution_result.outcome == "OUTCOME_OK";
                if let Some(output) = &code_execution_result.output {
                    result.execution_output.clone_from(output);
                }
                if !result.execution_success {
                    result.execution_error = Some(format!(
                        "Code execution failed: {}",
                        code_execution_result.outcome
                    ));
                }
            },
            GeminiPart::Text { text } => {
                if result.execution_output.is_empty() && !text.is_empty() {
                    info!(
                        text_preview = %text.chars().take(200).collect::<String>(),
                        "Text response (not code result)"
                    );
                }
            },
            _ => {},
        }
    }

    Ok(result)
}

fn build_code_execution_response(
    result: CodeExtractionResult,
    elapsed: Duration,
) -> CodeExecutionResponse {
    CodeExecutionResponse {
        generated_code: result.generated_code,
        execution_output: result.execution_output,
        success: result.execution_success,
        error: result.execution_error,
        latency_ms: elapsed.as_millis() as u64,
    }
}

fn get_first_candidate(response: &GeminiResponse) -> Result<&GeminiCandidate> {
    response
        .candidates
        .first()
        .ok_or_else(|| anyhow!("No response from Gemini for code execution"))
}

fn log_completion(request_id: Uuid, result: &CodeExtractionResult, latency_ms: u64) {
    info!(
        request_id = %request_id,
        success = result.execution_success,
        code_length = result.generated_code.len(),
        output_length = result.execution_output.len(),
        latency_ms = latency_ms,
        "Code execution complete"
    );
}

pub async fn generate_with_code_execution(
    provider: &GeminiProvider,
    messages: &[AiMessage],
    sampling: Option<&SamplingParams>,
    max_output_tokens: u32,
    model: &str,
) -> Result<CodeExecutionResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    info!(request_id = %request_id, model = %model, "Sending code execution request");

    let request = build_code_execution_request(messages, sampling, max_output_tokens);
    let gemini_response = send_and_parse_request(provider, &request, model, request_id).await?;
    let candidate = get_first_candidate(&gemini_response)?;
    let execution_result = extract_code_execution_result(candidate)?;

    log_completion(
        request_id,
        &execution_result,
        start.elapsed().as_millis() as u64,
    );

    Ok(build_code_execution_response(
        execution_result,
        start.elapsed(),
    ))
}
