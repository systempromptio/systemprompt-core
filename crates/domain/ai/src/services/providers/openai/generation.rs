use anyhow::{anyhow, Result};
use serde_json::json;
use std::time::Instant;
use tracing::warn;
use uuid::Uuid;

use crate::models::ai::AiResponse;
use crate::models::providers::openai::{
    OpenAiJsonSchema, OpenAiRequest, OpenAiResponse, OpenAiResponseFormat,
};
use crate::models::tools::ToolCall;
use crate::services::providers::{
    GenerationParams, SchemaGenerationParams, StructuredGenerationParams, ToolGenerationParams,
};
use systemprompt_identifiers::AiToolCallId;

use super::provider::OpenAiProvider;
use super::response_builder::build_response;
use super::{converters, reasoning};

pub async fn generate(
    provider: &OpenAiProvider,
    params: GenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
        params.messages.iter().map(Into::into).collect();

    let (temperature, top_p, presence_penalty, frequency_penalty) =
        params.sampling.map_or((None, None, None, None), |s| {
            (
                s.temperature,
                s.top_p,
                s.presence_penalty,
                s.frequency_penalty,
            )
        });

    let reasoning_config = reasoning::build_reasoning_config(params.model);

    let request = OpenAiRequest {
        model: params.model.to_string(),
        messages: openai_messages,
        temperature,
        top_p,
        presence_penalty,
        frequency_penalty,
        max_tokens: Some(params.max_output_tokens),
        tools: None,
        response_format: None,
        reasoning_effort: reasoning_config,
    };

    let response = provider
        .client
        .post(format!("{}/chat/completions", provider.endpoint))
        .bearer_auth(&provider.api_key)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("OpenAI API error: {error_text}"));
    }

    let openai_response: OpenAiResponse = response.json().await?;
    build_response(request_id, &openai_response, "openai", params.model, start)
}

pub async fn generate_with_tools(
    provider: &OpenAiProvider,
    params: ToolGenerationParams<'_>,
) -> Result<(AiResponse, Vec<ToolCall>)> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
        params.base.messages.iter().map(Into::into).collect();

    let openai_tools = converters::convert_tools(params.tools)?;

    let (temperature, top_p, presence_penalty, frequency_penalty) =
        params.base.sampling.map_or((None, None, None, None), |s| {
            (
                s.temperature,
                s.top_p,
                s.presence_penalty,
                s.frequency_penalty,
            )
        });

    let reasoning_config = reasoning::build_reasoning_config(params.base.model);

    let request = OpenAiRequest {
        model: params.base.model.to_string(),
        messages: openai_messages,
        temperature,
        top_p,
        presence_penalty,
        frequency_penalty,
        max_tokens: Some(params.base.max_output_tokens),
        tools: Some(openai_tools),
        response_format: None,
        reasoning_effort: reasoning_config,
    };

    let response = provider
        .client
        .post(format!("{}/chat/completions", provider.endpoint))
        .bearer_auth(&provider.api_key)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("OpenAI API error: {error_text}"));
    }

    let openai_response: OpenAiResponse = response.json().await?;

    let choice = openai_response
        .choices
        .first()
        .ok_or_else(|| anyhow!("No response from OpenAI"))?;

    let tool_calls = choice
        .message
        .tool_calls
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|tc| {
            let arguments = serde_json::from_str::<serde_json::Value>(&tc.function.arguments)
                .unwrap_or_else(|e| {
                    warn!(
                        error = %e,
                        tool_name = %tc.function.name,
                        raw_arguments = %tc.function.arguments,
                        "Failed to parse OpenAI tool arguments"
                    );
                    json!({})
                });

            ToolCall {
                ai_tool_call_id: AiToolCallId::from(tc.id),
                name: tc.function.name,
                arguments,
            }
        })
        .collect();

    let ai_response = build_response(
        request_id,
        &openai_response,
        "openai",
        params.base.model,
        start,
    )?;
    Ok((ai_response, tool_calls))
}

pub async fn generate_structured(
    provider: &OpenAiProvider,
    params: StructuredGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
        params.base.messages.iter().map(Into::into).collect();

    let (temperature, top_p, presence_penalty, frequency_penalty) =
        params.base.sampling.map_or((None, None, None, None), |s| {
            (
                s.temperature,
                s.top_p,
                s.presence_penalty,
                s.frequency_penalty,
            )
        });

    let reasoning_config = reasoning::build_reasoning_config(params.base.model);

    let request = OpenAiRequest {
        model: params.base.model.to_string(),
        messages: openai_messages,
        temperature,
        top_p,
        presence_penalty,
        frequency_penalty,
        max_tokens: Some(params.base.max_output_tokens),
        tools: None,
        response_format: converters::convert_response_format(params.response_format)?,
        reasoning_effort: reasoning_config,
    };

    let response = provider
        .client
        .post(format!("{}/chat/completions", provider.endpoint))
        .bearer_auth(&provider.api_key)
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("OpenAI API error: {error_text}"));
    }

    let openai_response: OpenAiResponse = response.json().await?;
    build_response(
        request_id,
        &openai_response,
        "openai",
        params.base.model,
        start,
    )
}

pub async fn generate_with_schema(
    provider: &OpenAiProvider,
    params: SchemaGenerationParams<'_>,
) -> Result<AiResponse> {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    let openai_messages: Vec<crate::models::providers::openai::OpenAiMessage> =
        params.base.messages.iter().map(Into::into).collect();

    let (temperature, top_p) = params
        .base
        .sampling
        .map_or((None, None), |s| (s.temperature, s.top_p));

    let reasoning_config = reasoning::build_reasoning_config(params.base.model);

    let request = OpenAiRequest {
        model: params.base.model.to_string(),
        messages: openai_messages,
        temperature,
        top_p,
        presence_penalty: None,
        frequency_penalty: None,
        max_tokens: Some(params.base.max_output_tokens),
        tools: None,
        response_format: Some(OpenAiResponseFormat::JsonSchema {
            json_schema: OpenAiJsonSchema {
                name: "structured_output".to_string(),
                schema: params.response_schema,
                strict: Some(true),
            },
        }),
        reasoning_effort: reasoning_config,
    };

    let response = provider
        .client
        .post(format!("{}/chat/completions", provider.endpoint))
        .header("Authorization", format!("Bearer {}", provider.api_key))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("OpenAI API error: {error_text}"));
    }

    let openai_response: OpenAiResponse = response.json().await?;
    build_response(
        request_id,
        &openai_response,
        "openai",
        params.base.model,
        start,
    )
}
