//! The reader's notification feed over the bridge JWT: catch-up since a
//! cursor, mark-read, seen receipts and escalate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_identifiers::NotificationId;
use systemprompt_models::ApiPaths;
use systemprompt_models::notifications::{
    Escalate, KeysetCursor, MarkRead, MarkSeen, Notification, NotificationPage,
    NotificationReadPayload,
};

use crate::gateway::errors::GatewayError;
use crate::gateway::{GatewayClient, ensure_success, record_span};
use crate::ids::BearerToken;

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, since),
        fields(endpoint = "notifications", status, latency_ms)
    )]
    pub async fn fetch_notifications(
        &self,
        bearer: &BearerToken,
        since: Option<&KeysetCursor>,
        limit: u32,
        unread_only: bool,
    ) -> Result<NotificationPage, GatewayError> {
        let mut url = url::Url::parse(&self.url(ApiPaths::BRIDGE_NOTIFICATIONS))
            .map_err(GatewayError::NotificationsUrl)?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(since) = since {
                query.append_pair("since", &since.to_string());
            }
            query.append_pair("limit", &limit.to_string());
            if unread_only {
                query.append_pair("unread", "true");
            }
        }
        let started = Instant::now();
        let resp = self
            .http()
            .get(url.as_str())
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::NotificationsRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "notifications").await?;
        resp.json()
            .await
            .map_err(|e| GatewayError::NotificationsDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, body),
        fields(endpoint = "notifications.read", status, latency_ms)
    )]
    pub async fn mark_notifications_read(
        &self,
        bearer: &BearerToken,
        body: &MarkRead,
    ) -> Result<NotificationReadPayload, GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .post(self.url(ApiPaths::BRIDGE_NOTIFICATIONS_READ))
            .bearer_auth(bearer.expose())
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::NotificationsRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "notifications.read").await?;
        resp.json()
            .await
            .map_err(|e| GatewayError::NotificationsDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, body),
        fields(endpoint = "notifications.seen", status, latency_ms)
    )]
    pub async fn mark_notifications_seen(
        &self,
        bearer: &BearerToken,
        body: &MarkSeen,
    ) -> Result<(), GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .post(self.url(ApiPaths::BRIDGE_NOTIFICATIONS_SEEN))
            .bearer_auth(bearer.expose())
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::NotificationsRequest(Box::new(e)))?;
        record_span(&resp, started);
        ensure_success(resp, "notifications.seen").await?;
        Ok(())
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, body),
        fields(endpoint = "notifications.escalate", status, latency_ms)
    )]
    pub async fn escalate_notification(
        &self,
        bearer: &BearerToken,
        id: &NotificationId,
        body: &Escalate,
    ) -> Result<Notification, GatewayError> {
        let path = format!(
            "{base}/{id}/escalate",
            base = ApiPaths::BRIDGE_NOTIFICATIONS,
            id = id.as_str()
        );
        let started = Instant::now();
        let resp = self
            .http()
            .post(self.url(&path))
            .bearer_auth(bearer.expose())
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::NotificationsRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "notifications.escalate").await?;
        resp.json()
            .await
            .map_err(|e| GatewayError::NotificationsDecode(Box::new(e)))
    }
}
