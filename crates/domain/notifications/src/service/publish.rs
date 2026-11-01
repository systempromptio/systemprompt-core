//! Publishing to a stream: every publish policy must admit the publisher,
//! then the event is appended to the outbox in its own transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use systemprompt_extension::notifications::PublishVerdict;
use systemprompt_identifiers::{NotificationStreamId, UserId};
use systemprompt_models::notifications::{
    Attention, NotificationEvent, NotificationKind, SubjectRef,
};

use super::NotificationsService;
use crate::error::{NotificationsError, NotificationsResult};

/// One item to publish on a stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishRequest {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub subjects: Vec<SubjectRef>,
    #[serde(default)]
    pub attention: Option<Attention>,
}

/// Whether the item reached the outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishOutcome {
    pub enqueued: bool,
}

impl NotificationsService {
    pub async fn publish(
        &self,
        user: &UserId,
        stream: &NotificationStreamId,
        request: PublishRequest,
    ) -> NotificationsResult<PublishOutcome> {
        let PublishRequest {
            title,
            body,
            subjects,
            attention,
        } = request;
        let declared = self
            .repos
            .streams
            .find_pooled(stream)
            .await?
            .ok_or_else(|| NotificationsError::StreamNotFound {
                stream: stream.clone(),
            })?;
        for policy in &self.publish_policies {
            if let PublishVerdict::Refuse { reason } = policy.may_publish(user, &declared).await? {
                return Err(NotificationsError::PublishRefused { reason });
            }
        }
        let kind = NotificationKind::new(NotificationKind::STREAM_PUBLISHED)?;
        let mut event = NotificationEvent::new(kind, user.clone(), Utc::now())
            .with_stream(stream.clone())
            .with_subjects(subjects)
            .with_title(title);
        if let Some(body) = body {
            event = event.with_body(body);
        }
        if let Some(attention) = attention {
            event = event.with_sender_attention(attention);
        }
        let mut tx = self.db.write_pool().begin().await?;
        self.sink.emit(&mut tx, &event).await?;
        tx.commit().await?;
        Ok(PublishOutcome { enqueued: true })
    }
}
