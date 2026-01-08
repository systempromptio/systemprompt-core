use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionId, SessionToken};

#[derive(Debug, Serialize)]
struct TuiSessionRequest {
    client_id: String,
    user_id: String,
    email: String,
}

#[derive(Debug, Deserialize)]
struct TuiSessionResponse {
    session_id: String,
}

pub async fn create_tui_session(api_url: &str, user_id: &str, email: &str) -> Result<SessionId> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = TuiSessionRequest {
        client_id: "sp_tui".to_string(),
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

    let session_response: TuiSessionResponse = response
        .json()
        .await
        .context("Failed to parse session response")?;

    Ok(SessionId::new(session_response.session_id))
}

pub const fn end_tui_session(_session_id: &SessionId) {}

pub async fn verify_token(api_url: &str, token: &SessionToken) -> Result<bool> {
    let client = SystempromptClient::new(api_url)?.with_token(JwtToken::new(token.as_str()));
    client
        .verify_token()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to verify token: {}", e))
}

pub async fn check_health(api_url: &str) -> bool {
    let Ok(client) = SystempromptClient::new(api_url) else {
        return false;
    };
    client.check_health().await
}
