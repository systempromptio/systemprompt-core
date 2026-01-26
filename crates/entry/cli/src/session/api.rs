use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::SessionId;

#[derive(Debug, Serialize)]
struct CliSessionRequest {
    client_id: String,
    user_id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct CliSessionResponse {
    session_id: String,
}

pub(super) async fn request_session_id(
    api_url: &str,
    user_id: &str,
    email: &str,
) -> Result<SessionId> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = CliSessionRequest {
        client_id: "sp_cli".to_string(),
        user_id: user_id.to_string(),
        email: email.to_string(),
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Failed to send session creation request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));
        anyhow::bail!("Session creation failed with status {}: {}", status, body);
    }

    let session_response: CliSessionResponse = response
        .json()
        .await
        .context("Failed to parse session response")?;

    Ok(SessionId::new(session_response.session_id))
}
