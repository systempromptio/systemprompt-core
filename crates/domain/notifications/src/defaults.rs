//! The core seams every deployment gets: [`DefaultRouter`] and
//! [`DefaultPublishPolicy`].
//!
//! The default router addresses only an event's direct recipients: a mention
//! defaults to `human`, a share to `agent`, a stream publication to nobody
//! (its stream audience is expanded by the pipeline) and any other kind to
//! `silent` unless the producer chose otherwise.
//!
//! The default publish policy admits a publisher the stream declares, as a
//! user, or as a role or dimension the registered audience resolvers answer
//! [`Membership::Member`] for; it never lists an audience's members.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::notifications::{
    Membership, NotificationAudienceResolver, NotificationRouter, NotificationRouterError,
    PublishPolicy, PublishPolicyError, PublishVerdict,
};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::{
    Attention, AttentionReason, AudienceSubject, NotificationEvent, NotificationKind,
    RecipientDecision, RoutePlan, StreamDecl,
};

/// Routes an event to its direct recipients.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultRouter;

#[async_trait]
impl NotificationRouter for DefaultRouter {
    fn name(&self) -> &'static str {
        "core-default"
    }

    async fn route(&self, event: &NotificationEvent) -> Result<RoutePlan, NotificationRouterError> {
        let fallback = match event.kind.as_str() {
            NotificationKind::STREAM_PUBLISHED => {
                return Ok(RoutePlan {
                    recipients: Vec::new(),
                    stream: event.stream.clone(),
                });
            },
            NotificationKind::MENTIONED => event.sender_attention.unwrap_or(Attention::Human),
            NotificationKind::SHARED => Attention::Agent,
            _ => event.sender_attention.unwrap_or(Attention::Silent),
        };
        let shared = event.kind.as_str() == NotificationKind::SHARED;
        let recipients = event
            .direct_recipients
            .iter()
            .map(|recipient| {
                let chosen = if shared { None } else { recipient.attention };
                let reason = if chosen.is_some() || (!shared && event.sender_attention.is_some()) {
                    AttentionReason::Sender
                } else {
                    AttentionReason::Default
                };
                RecipientDecision {
                    user: recipient.user.clone(),
                    attention: chosen.unwrap_or(fallback),
                    reason,
                }
            })
            .collect();
        Ok(RoutePlan {
            recipients,
            stream: event.stream.clone(),
        })
    }
}

/// Admits the publishers a stream declares.
#[derive(Clone)]
pub struct DefaultPublishPolicy {
    audiences: Vec<Arc<dyn NotificationAudienceResolver>>,
}

impl std::fmt::Debug for DefaultPublishPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultPublishPolicy")
            .field("audiences", &self.audiences.len())
            .finish()
    }
}

impl DefaultPublishPolicy {
    #[must_use]
    pub const fn new(audiences: Vec<Arc<dyn NotificationAudienceResolver>>) -> Self {
        Self { audiences }
    }

    async fn is_member(
        &self,
        subject: &AudienceSubject,
        publisher: &UserId,
    ) -> Result<bool, PublishPolicyError> {
        let Some(resolver) = self.audiences.iter().find(|r| r.handles(subject)) else {
            return Ok(false);
        };
        let membership = resolver
            .is_member(subject, publisher)
            .await
            .map_err(|error| PublishPolicyError::Other {
                policy: "stream-publishers",
                source: Box::new(error),
            })?;
        Ok(membership == Membership::Member)
    }
}

#[async_trait]
impl PublishPolicy for DefaultPublishPolicy {
    fn name(&self) -> &'static str {
        "stream-publishers"
    }

    async fn may_publish(
        &self,
        publisher: &UserId,
        stream: &StreamDecl,
    ) -> Result<PublishVerdict, PublishPolicyError> {
        for subject in &stream.publishers {
            let admitted = match subject {
                AudienceSubject::Everyone => true,
                AudienceSubject::User(user) => user == publisher,
                other => self.is_member(other, publisher).await?,
            };
            if admitted {
                return Ok(PublishVerdict::Admit);
            }
        }
        Ok(PublishVerdict::Refuse {
            reason: "not a declared publisher".to_owned(),
        })
    }
}
