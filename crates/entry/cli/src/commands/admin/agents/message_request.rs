use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use systemprompt_agent::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, JsonRpcResponse, Request};
use systemprompt_agent::models::a2a::protocol::MessageSendParams;
use systemprompt_models::a2a::Task;

use super::message::extract_text_from_parts;
use super::types::MessageOutput;
use crate::shared::CommandResult;

pub(super) struct NonStreamingRequest<'a> {
    pub agent: &'a str,
    pub agent_url: &'a str,
    pub auth_token: &'a str,
    pub request: &'a Request<MessageSendParams>,
    pub message_text: &'a str,
    pub timeout: u64,
}

pub(super) async fn execute_non_streaming(
    params: NonStreamingRequest<'_>,
) -> Result<CommandResult<MessageOutput>> {
    let NonStreamingRequest {
        agent,
        agent_url,
        auth_token,
        request,
        message_text,
        timeout,
    } = params;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .post(agent_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(request)
        .send()
        .await
        .with_context(|| format!("Failed to send message to agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Agent request failed with status {}: {}", status, body);
    }

    let json_response: JsonRpcResponse<Task> = response
        .json()
        .await
        .context("Failed to parse agent response")?;

    if json_response.jsonrpc != JSON_RPC_VERSION_2_0 {
        anyhow::bail!(
            "Invalid JSON-RPC version: expected {}, got {}",
            JSON_RPC_VERSION_2_0,
            json_response.jsonrpc
        );
    }

    if let Some(error) = json_response.error {
        let details = error
            .data
            .map_or_else(String::new, |d| format!("\n\nDetails: {}", d));
        anyhow::bail!(
            "Agent returned error ({}): {}{}",
            error.code,
            error.message,
            details
        );
    }

    let task = json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))?;

    let response = task
        .status
        .message
        .as_ref()
        .map(|msg| extract_text_from_parts(&msg.parts));

    let output = MessageOutput {
        agent: agent.to_owned(),
        task,
        message_sent: message_text.to_owned(),
        response,
    };

    Ok(CommandResult::card(output).with_title(format!("Message sent to {}", agent)))
}
