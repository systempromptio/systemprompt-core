use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use systemprompt_core_logging::CliService;
use uuid::Uuid;

use super::TraceOptions;

pub async fn get_first_agent(client: &Client, base_url: &str, token: &str) -> Result<String> {
    let response = client
        .get(format!("{}/api/v1/agents/registry", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
        .context("Failed to fetch agent registry")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch agent registry: {}", response.status());
    }

    let registry: Value = response.json().await?;
    let agents = registry
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("No agents array in registry response"))?;

    if agents.is_empty() {
        anyhow::bail!("No agents found in registry. Is the API running?");
    }

    agents[0]
        .get("name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("No agent name found in registry"))
}

pub async fn get_anonymous_token(client: &Client, base_url: &str) -> Result<String> {
    let response = client
        .post(format!("{}/api/v1/core/oauth/session", base_url))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
        .context("Failed to get anonymous token")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));
        anyhow::bail!("Failed to get anonymous token: {} - {}", status, body);
    }

    let body: Value = response.json().await?;
    body.get("access_token")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("No access_token in auth response: {:?}", body))
}

pub async fn create_context(client: &Client, base_url: &str, token: &str) -> Result<String> {
    let response = client
        .post(format!("{}/api/v1/core/contexts", base_url))
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&json!({
            "name": "CLI Trace Test"
        }))
        .send()
        .await
        .context("Failed to create context")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to create context: {}", response.status());
    }

    let body: Value = response.json().await?;
    body.get("context_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("No context_id in response"))
}

pub struct MessageContext<'a> {
    pub base_url: &'a str,
    pub agent_name: &'a str,
    pub token: &'a str,
    pub trace_id: &'a str,
    pub context_id: &'a str,
}

pub async fn send_test_message(
    client: &Client,
    ctx: &MessageContext<'_>,
    message: &str,
) -> Result<()> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method": "message/send",
        "params": {
            "message": {
                "contextId": ctx.context_id,
                "messageId": Uuid::new_v4().to_string(),
                "role": "user",
                "kind": "message",
                "parts": [
                    {
                        "kind": "text",
                        "text": message
                    }
                ]
            }
        },
        "id": Uuid::new_v4().to_string()
    });

    let response = client
        .post(format!("{}/api/v1/agents/{}/", ctx.base_url, ctx.agent_name))
        .header("Authorization", format!("Bearer {}", ctx.token))
        .header("x-trace-id", ctx.trace_id)
        .header("Content-Type", "application/json")
        .timeout(Duration::from_secs(300))
        .json(&payload)
        .send()
        .await
        .context("Failed to send message to agent")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));
        anyhow::bail!("Agent request failed: {} - {}", status, body);
    }

    Ok(())
}

pub async fn send_and_trace(options: &TraceOptions, base_url: &str) -> Result<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(360))
        .build()?;

    let trace_id = Uuid::new_v4().to_string();
    let message = options.message.as_deref().unwrap_or("Hello");

    CliService::section("Sending test message...");
    CliService::key_value("  Trace ID", &trace_id);

    let token = get_anonymous_token(&client, base_url)
        .await
        .context("Failed to authenticate")?;
    CliService::success("  [OK] Got anonymous token");

    let agent_name = if let Some(ref agent) = options.agent {
        agent.clone()
    } else {
        get_first_agent(&client, base_url, &token).await?
    };
    CliService::success(&format!("  [OK] Using agent: {agent_name}"));

    let context_id = create_context(&client, base_url, &token).await?;
    CliService::success(&format!(
        "  [OK] Created context: {}...",
        &context_id[..context_id.len().min(8)]
    ));

    CliService::info(&format!("  -> Sending message: \"{message}\""));

    let msg_ctx = MessageContext {
        base_url,
        agent_name: &agent_name,
        token: &token,
        trace_id: &trace_id,
        context_id: &context_id,
    };
    send_test_message(&client, &msg_ctx, message).await?;

    CliService::success("  [OK] Message sent, waiting for processing...");

    tokio::time::sleep(Duration::from_millis(500)).await;

    Ok(trace_id)
}
