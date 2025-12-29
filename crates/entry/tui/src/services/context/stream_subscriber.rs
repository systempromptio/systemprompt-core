use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::StreamExt;
use reqwest::Client;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use systemprompt_identifiers::{ContextId, JwtToken};
use systemprompt_models::modules::ApiPaths;

use super::event_parser;
use crate::messages::Message;
use crate::state::SseStatus;

const MAX_RECONNECT_DELAY_SECS: u64 = 32;
const INITIAL_RECONNECT_DELAY_SECS: u64 = 2;

pub struct ContextStreamSubscriber {
    message_tx: UnboundedSender<Message>,
    auth_token: JwtToken,
    api_base_url: String,
    current_context_id: Arc<RwLock<ContextId>>,
}

impl ContextStreamSubscriber {
    pub fn new_with_url(
        message_tx: UnboundedSender<Message>,
        auth_token: JwtToken,
        current_context_id: Arc<RwLock<ContextId>>,
        api_base_url: &str,
    ) -> Self {
        Self {
            message_tx,
            auth_token,
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
            current_context_id,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(&self) {
        info!(api_url = %self.api_base_url, "Context stream task started");

        let mut reconnect_delay = INITIAL_RECONNECT_DELAY_SECS;
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            self.send_connection_status(attempt);
            reconnect_delay = self.attempt_connection(reconnect_delay, attempt).await;
            tokio::time::sleep(Duration::from_secs(reconnect_delay)).await;
            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY_SECS);
        }
    }

    fn send_connection_status(&self, attempt: u32) {
        let status = if attempt == 1 {
            SseStatus::Connecting
        } else {
            SseStatus::Reconnecting
        };
        let _ = self.message_tx.send(Message::SseStatusUpdate(status));
    }

    async fn attempt_connection(&self, reconnect_delay: u64, attempt: u32) -> u64 {
        match self.connect_and_stream().await {
            Ok(()) => {
                self.handle_clean_disconnect();
                INITIAL_RECONNECT_DELAY_SECS
            },
            Err(e) => {
                self.handle_connection_error(&e, reconnect_delay, attempt);
                reconnect_delay
            },
        }
    }

    fn handle_clean_disconnect(&self) {
        info!("Context stream disconnected cleanly");
        let _ = self
            .message_tx
            .send(Message::SseStatusUpdate(SseStatus::Disconnected));
    }

    fn handle_connection_error(&self, error: &anyhow::Error, reconnect_delay: u64, attempt: u32) {
        error!(
            error = %error,
            reconnect_delay = reconnect_delay,
            attempt = attempt,
            "Context stream error, reconnecting"
        );

        let status = if attempt >= 3 {
            SseStatus::Failed
        } else {
            SseStatus::Disconnected
        };
        let _ = self.message_tx.send(Message::SseStatusUpdate(status));
    }

    async fn connect_and_stream(&self) -> anyhow::Result<()> {
        let client = Self::build_http_client()?;
        let response = self.send_sse_request(&client).await?;
        self.log_connection_established().await;
        self.stream_events(response).await
    }

    fn build_http_client() -> anyhow::Result<Client> {
        Client::builder()
            .http1_only()
            .tcp_nodelay(true)
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))
    }

    async fn send_sse_request(&self, client: &Client) -> anyhow::Result<reqwest::Response> {
        let url = format!("{}{}", self.api_base_url, ApiPaths::STREAM_CONTEXTS);
        info!(url = %url, "Connecting to Context Stream");

        let response = client
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", self.auth_token.as_str()),
            )
            .header("Accept", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .send()
            .await?;

        let status = response.status();
        info!(status = %status, "SSE response received");

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "AG-UI stream connection failed: {}",
                status
            ));
        }

        Ok(response)
    }

    async fn log_connection_established(&self) {
        let current_ctx = self.current_context_id.read().await;
        info!(
            context_id = %current_ctx.as_str(),
            "Connected to Context Stream"
        );
        drop(current_ctx);

        let _ = self
            .message_tx
            .send(Message::SseStatusUpdate(SseStatus::Connected));
    }

    async fn stream_events(&self, response: reqwest::Response) -> anyhow::Result<()> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut chunk_count = 0u64;

        debug!("Context stream started, waiting for events...");

        while let Some(chunk_result) = stream.next().await {
            chunk_count += 1;
            self.handle_stream_chunk(chunk_result, &mut buffer, chunk_count)?;
        }

        warn!(chunk_count = chunk_count, "SSE stream ended");
        Ok(())
    }

    fn handle_stream_chunk(
        &self,
        chunk_result: Result<Bytes, reqwest::Error>,
        buffer: &mut String,
        chunk_count: u64,
    ) -> anyhow::Result<()> {
        match chunk_result {
            Ok(chunk) => {
                debug!(
                    chunk_num = chunk_count,
                    bytes = chunk.len(),
                    "SSE chunk received"
                );
                self.process_chunk(&chunk, buffer);
                Ok(())
            },
            Err(e) => {
                error!(
                    chunk_count = chunk_count,
                    error = %e,
                    "SSE stream read error"
                );
                Err(anyhow::anyhow!("Stream read error: {}", e))
            },
        }
    }

    fn process_chunk(&self, chunk: &[u8], buffer: &mut String) {
        let chunk_str = match std::str::from_utf8(chunk) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "Invalid UTF-8 in SSE chunk");
                return;
            },
        };

        buffer.push_str(chunk_str);

        while let Some(event_end) = buffer.find("\n\n") {
            let event_str = buffer[..event_end].to_string();
            *buffer = buffer[event_end + 2..].to_string();

            self.dispatch_event(&event_str);
        }
    }

    fn dispatch_event(&self, event_str: &str) {
        let data = event_str
            .strip_prefix("data: ")
            .or_else(|| event_str.strip_prefix("data:"))
            .unwrap_or(event_str)
            .trim();

        event_parser::parse_and_dispatch(data, &self.message_tx);
    }
}

impl std::fmt::Debug for ContextStreamSubscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextStreamSubscriber")
            .field("api_base_url", &self.api_base_url)
            .field("current_context_id", &"<Arc<RwLock<ContextId>>>")
            .finish_non_exhaustive()
    }
}
