use std::time::Duration;

use tokio::sync::mpsc;

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionToken};

use crate::messages::{LogEntry, Message};

pub struct LogStreamer {
    api_url: String,
    token: SessionToken,
    message_tx: mpsc::UnboundedSender<Message>,
    poll_interval: Duration,
    last_timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl LogStreamer {
    pub const fn new(
        api_url: String,
        token: SessionToken,
        message_tx: mpsc::UnboundedSender<Message>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            api_url,
            token,
            message_tx,
            poll_interval,
            last_timestamp: None,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut this = self;
            this.run().await;
        })
    }

    async fn run(&mut self) {
        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Some(entries) = self.fetch_new_logs().await {
                if !entries.is_empty() {
                    let _ = self.message_tx.send(Message::LogsBatch(entries));
                }
            }
        }
    }

    async fn fetch_new_logs(&mut self) -> Option<Vec<LogEntry>> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(JwtToken::new(self.token.as_str())),
            Err(e) => {
                tracing::error!("Failed to create client: {}", e);
                return None;
            },
        };

        let logs = match client.list_logs(Some(100)).await {
            Ok(logs) => logs,
            Err(e) => {
                tracing::error!("Failed to fetch logs: {}", e);
                return None;
            },
        };

        let last_timestamp = self.last_timestamp;
        let is_initial_fetch = last_timestamp.is_none();

        let mut new_logs: Vec<LogEntry> = logs
            .into_iter()
            .filter(|log| last_timestamp.is_none_or(|timestamp| log.timestamp > timestamp))
            .map(|log| LogEntry {
                timestamp: log.timestamp,
                level: log.level,
                module: log.module.clone(),
                message: log.message,
            })
            .collect();

        new_logs.reverse();

        if is_initial_fetch && new_logs.len() > 20 {
            let skip_count = new_logs.len() - 20;
            new_logs = new_logs.into_iter().skip(skip_count).collect();
        }

        if let Some(newest_log) = new_logs.last() {
            self.last_timestamp = Some(newest_log.timestamp);
        }

        Some(new_logs)
    }
}

impl std::fmt::Debug for LogStreamer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LogStreamer")
            .field("poll_interval", &self.poll_interval)
            .field("last_timestamp", &self.last_timestamp)
            .finish_non_exhaustive()
    }
}
