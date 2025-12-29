use std::time::Duration;

use tokio::sync::mpsc;

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;

use crate::messages::Message;
use crate::state::UserDisplay;

pub struct UserPoller {
    api_url: String,
    token: JwtToken,
    message_tx: mpsc::UnboundedSender<Message>,
    poll_interval: Duration,
}

impl UserPoller {
    pub const fn new(
        api_url: String,
        token: JwtToken,
        message_tx: mpsc::UnboundedSender<Message>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            api_url,
            token,
            message_tx,
            poll_interval,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(&self) {
        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let users = self.fetch_users().await;
            let _ = self.message_tx.send(Message::UsersUpdate(users));
        }
    }

    async fn fetch_users(&self) -> Vec<UserDisplay> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(self.token.clone()),
            Err(e) => {
                tracing::error!("Failed to create client: {}", e);
                return Vec::new();
            },
        };

        match client.list_users(Some(100)).await {
            Ok(users) => users
                .into_iter()
                .map(|u| UserDisplay {
                    id: u.id,
                    name: u.name,
                    sessions: u.active_sessions,
                    last_accessed: u.last_session_at,
                    roles: u.roles,
                })
                .collect(),
            Err(e) => {
                tracing::error!("Failed to fetch users: {}", e);
                Vec::new()
            },
        }
    }
}

impl std::fmt::Debug for UserPoller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserPoller")
            .field("poll_interval", &self.poll_interval)
            .finish_non_exhaustive()
    }
}
