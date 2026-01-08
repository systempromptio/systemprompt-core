use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use systemprompt_identifiers::{ContextId, SessionToken};
use tokio::sync::mpsc::UnboundedSender;

use crate::messages::Message;

#[derive(Clone)]
pub struct MessageSender {
    client: Client,
    api_base_url: String,
    session_token: SessionToken,
    message_tx: UnboundedSender<Message>,
}

impl MessageSender {
    pub fn new_with_url(
        session_token: SessionToken,
        message_tx: UnboundedSender<Message>,
        api_base_url: &str,
    ) -> Self {
        Self {
            client: Client::new(),
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            session_token,
            message_tx,
        }
    }

    pub async fn send(
        &self,
        agent_name: &str,
        context_id: &ContextId,
        content: &str,
    ) -> Result<()> {
        let url = format!("{}/api/v1/agents/{}", self.api_base_url, agent_name);

        let body = json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": "message/send",
            "params": {
                "message": {
                    "role": "user",
                    "parts": [{"kind": "text", "text": content}],
                    "messageId": uuid::Uuid::new_v4().to_string(),
                    "contextId": context_id.as_str(),
                    "kind": "message"
                }
            }
        });

        let response = self
            .client
            .post(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.session_token.as_str()),
            )
            .header("Content-Type", "application/json")
            .header("x-context-id", context_id.as_str())
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response
            .text()
            .await
            .unwrap_or_else(|e| format!("<error reading response: {}>", e));

        if !status.is_success() {
            let _ = self.message_tx.send(Message::TaskProgressError(format!(
                "Failed to send message: {} - {}",
                status, response_text
            )));
            return Err(anyhow::anyhow!(
                "Failed to send message: {} - {}",
                status,
                response_text
            ));
        }

        if let Ok(json_response) = serde_json::from_str::<serde_json::Value>(&response_text) {
            if let Some(error) = json_response.get("error") {
                let error_msg = error
                    .get("data")
                    .map(|d| d.as_str().map_or_else(|| d.to_string(), String::from))
                    .or_else(|| {
                        error
                            .get("message")
                            .map(|m| m.as_str().unwrap_or("Unknown error").to_string())
                    })
                    .unwrap_or_else(|| "Unknown JSON-RPC error".to_string());

                tracing::error!("JSON-RPC error received: {}", error_msg);
                let _ = self
                    .message_tx
                    .send(Message::TaskProgressError(error_msg.clone()));
                return Err(anyhow::anyhow!("JSON-RPC error: {}", error_msg));
            }
        }

        Ok(())
    }
}

impl std::fmt::Debug for MessageSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageSender")
            .field("api_base_url", &self.api_base_url)
            .finish_non_exhaustive()
    }
}
