use std::time::Duration;

use tokio::sync::mpsc;

use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::JwtToken;

use crate::messages::Message;
use crate::state::{AnalyticsData, TrafficData};

pub struct AnalyticsPoller {
    api_url: String,
    token: JwtToken,
    message_tx: mpsc::UnboundedSender<Message>,
    poll_interval: Duration,
}

impl AnalyticsPoller {
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
        if let Some(data) = self.fetch_analytics().await {
            let _ = self.message_tx.send(Message::AnalyticsUpdate(data));
        }

        let mut interval = tokio::time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Some(data) = self.fetch_analytics().await {
                let _ = self.message_tx.send(Message::AnalyticsUpdate(data));
            }
        }
    }

    async fn fetch_analytics(&self) -> Option<AnalyticsData> {
        let client = match SystempromptClient::new(&self.api_url) {
            Ok(c) => c.with_token(self.token.clone()),
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

impl std::fmt::Debug for AnalyticsPoller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsPoller")
            .field("poll_interval", &self.poll_interval)
            .finish_non_exhaustive()
    }
}
