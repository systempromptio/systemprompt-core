use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use systemprompt_agent::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, JsonRpcResponse};
use systemprompt_loader::ConfigLoader;

pub(super) fn ensure_agent_exists(name: &str) -> Result<()> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    if !services_config.agents.contains_key(name) {
        return Err(anyhow!("Agent '{}' not found", name));
    }
    Ok(())
}

pub(super) struct A2aCall<'a, T: Serialize> {
    pub agent: &'a str,
    pub agent_url: &'a str,
    pub auth_token: &'a str,
    pub request: &'a T,
    pub timeout: u64,
}

pub(super) async fn send_a2a_request<Req, Res>(call: A2aCall<'_, Req>) -> Result<Res>
where
    Req: Serialize + Sync,
    Res: DeserializeOwned,
{
    let A2aCall {
        agent,
        agent_url,
        auth_token,
        request,
        timeout,
    } = call;

    let client = Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .post(agent_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(request)
        .send()
        .await
        .with_context(|| format!("Failed to reach agent '{}' at {}", agent, agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Agent request failed with status {}: {}", status, body);
    }

    let json_response: JsonRpcResponse<Res> = response
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

    json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))
}
