use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ClientId, SessionId, UserId};
use systemprompt_models::net::HTTP_AUTH_VERIFY_TIMEOUT;

#[derive(Debug, Serialize)]
struct CliSessionRequest {
    client_id: ClientId,
    user_id: UserId,
    email: String,
}

#[derive(Debug, Deserialize)]
struct CliSessionResponse {
    session_id: SessionId,
}

pub(super) async fn request_session_id(
    api_url: &str,
    user: &UserId,
    email: &str,
) -> Result<SessionId> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_AUTH_VERIFY_TIMEOUT)
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "{}/api/v1/core/oauth/session",
        api_url.trim_end_matches('/')
    );

    let request = CliSessionRequest {
        client_id: ClientId::new("sp_cli"),
        user_id: user.clone(),
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

    Ok(session_response.session_id)
}
