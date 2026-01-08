use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{JwtToken, SessionToken};

use crate::events::{TuiEvent, TuiEventBus};
use crate::messages::Message;
use crate::state::{AnalyticsData, TrafficData};

pub struct AnalyticsSubscriber {
    api_url: String,
    token: SessionToken,
    message_tx: mpsc::UnboundedSender<Message>,
    event_bus: Arc<TuiEventBus>,
}

impl AnalyticsSubscriber {
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
        if let Some(data) = self.fetch_analytics().await {
            let _ = self.message_tx.send(Message::AnalyticsUpdate(data));
        }

        let mut rx = self.event_bus.subscribe();

        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(TuiEvent::AnalyticsUpdated) => {
                            if let Some(data) = self.fetch_analytics().await {
                                let _ = self.message_tx.send(Message::AnalyticsUpdate(data));
                            }
                        }
                        Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                () = tokio::time::sleep(Duration::from_secs(60)) => {
                    if let Some(data) = self.fetch_analytics().await {
                        let _ = self.message_tx.send(Message::AnalyticsUpdate(data));
                    }
                }
            }
        }
    }

    async fn fetch_analytics(&self) -> Option<AnalyticsData> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(JwtToken::new(self.token.as_str())),
            Err(e) => {
                tracing::error!("Failed to create client: {}", e);
                return None;
            },
        };

        let analytics = match client.get_analytics().await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to fetch analytics: {}", e);
                return None;
            },
        };

        let traffic_data = analytics.traffic.map(|t| TrafficData {
            browsers: t.browsers,
            devices: t.devices,
            countries: t.countries,
            bot_traffic: t.bot_traffic,
        });

        Some(AnalyticsData {
            user_metrics: analytics.user_metrics,
            content_stats: analytics.content_stats,
            recent_conversations: analytics.recent_conversations,
            activity_trends: analytics.activity_trends,
            traffic_data,
        })
    }
}

impl std::fmt::Debug for AnalyticsSubscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsSubscriber")
            .finish_non_exhaustive()
    }
}
