use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionToken};

use crate::events::{TuiEvent, TuiEventBus};
use crate::messages::Message;
use crate::state::UserDisplay;

pub struct UserSubscriber {
    api_url: String,
    token: SessionToken,
    message_tx: mpsc::UnboundedSender<Message>,
    event_bus: Arc<TuiEventBus>,
}

impl UserSubscriber {
    pub const fn new(
        api_url: String,
        token: SessionToken,
        message_tx: mpsc::UnboundedSender<Message>,
        event_bus: Arc<TuiEventBus>,
    ) -> Self {
        Self {
            api_url,
            token,
            message_tx,
            event_bus,
        }
    }

    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(&self) {
        let users = self.fetch_users().await;
        let _ = self.message_tx.send(Message::UsersUpdate(users));

        let mut rx = self.event_bus.subscribe();

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(TuiEvent::UserChanged { .. } | TuiEvent::SessionChanged { .. }) => {
                            let users = self.fetch_users().await;
                            let _ = self.message_tx.send(Message::UsersUpdate(users));
                        }
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                () = tokio::time::sleep(Duration::from_secs(60)) => {
                    let users = self.fetch_users().await;
                    let _ = self.message_tx.send(Message::UsersUpdate(users));
                }
            }
        }
    }

    async fn fetch_users(&self) -> Vec<UserDisplay> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(JwtToken::new(self.token.as_str())),
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

impl std::fmt::Debug for UserSubscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserSubscriber").finish_non_exhaustive()
    }
}
