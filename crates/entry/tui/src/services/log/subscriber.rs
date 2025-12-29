use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;

use crate::messages::{LogEntry, Message};
use crate::state::ActiveTab;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

pub struct LogSubscriber {
    api_url: String,
    token: JwtToken,
    message_tx: mpsc::UnboundedSender<Message>,
    last_poll: Option<Instant>,
}

impl LogSubscriber {
    pub const fn new(
        api_url: String,
        token: JwtToken,
        message_tx: mpsc::UnboundedSender<Message>,
    ) -> Self {
        Self {
            api_url,
            token,
            message_tx,
            last_poll: None,
        }
    }

    pub fn should_poll(&self, active_tab: ActiveTab) -> bool {
        if active_tab != ActiveTab::Logs {
            return false;
        }

        self.last_poll
            .map_or(true, |last| last.elapsed() >= POLL_INTERVAL)
    }

    pub async fn poll_logs(&mut self) {
        self.last_poll = Some(Instant::now());

        if let Some(entries) = self.fetch_logs().await {
            if !entries.is_empty() {
                let _ = self.message_tx.send(Message::LogsBatch(entries));
            }
        }
    }

    pub async fn refresh(&mut self) {
        self.last_poll = Some(Instant::now());

        if let Some(entries) = self.fetch_logs().await {
            let _ = self.message_tx.send(Message::LogsBatch(entries));
        }
    }

    async fn fetch_logs(&self) -> Option<Vec<LogEntry>> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(self.token.clone()),
            Err(e) => {
                tracing::error!("Failed to create client: {}", e);
                return None;
            },
        };

        let logs = match client.list_logs(Some(1000)).await {
            Ok(logs) => logs,
            Err(e) => {
                tracing::error!("Failed to fetch logs: {}", e);
                return None;
            },
        };

        let mut entries: Vec<LogEntry> = logs
            .into_iter()
            .map(|log| LogEntry {
                timestamp: log.timestamp,
                level: log.level,
                module: log.module.clone(),
                message: log.message,
            })
            .collect();

        entries.reverse();

        Some(entries)
    }
}

impl std::fmt::Debug for LogSubscriber {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LogSubscriber")
            .finish_non_exhaustive()
    }
}
