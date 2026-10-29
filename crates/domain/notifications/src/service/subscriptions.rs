//! A user's own subscriptions. The subject is always the caller; declared
//! subscriptions to roles and dimensions come from the config plane. A
//! stream subscription requires the caller to be a member of the stream's
//! audience, and a subscription the configuration declares cannot be
//! overwritten from the console.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::ChangeSource;
use systemprompt_identifiers::{NotificationSubscriptionId, UserId};
use systemprompt_models::notifications::{AudienceSubject, SubscriptionDecl, SubscriptionTarget};

use super::NotificationsService;
use crate::error::{NotificationsError, NotificationsResult};
use crate::models::SubscriptionView;

impl NotificationsService {
    pub async fn subscriptions(&self, user: &UserId) -> NotificationsResult<Vec<SubscriptionView>> {
        Ok(self.repos.subscriptions.list_for_user(user).await?)
    }

    pub async fn subscribe(
        &self,
        user: &UserId,
        decl: SubscriptionDecl,
    ) -> NotificationsResult<SubscriptionView> {
        let decl = SubscriptionDecl {
            subject: AudienceSubject::User(user.clone()),
            ..decl
        };
        if let SubscriptionTarget::Stream(stream) = &decl.target {
            let declared = self
                .repos
                .streams
                .find_pooled(stream)
                .await?
                .ok_or_else(|| NotificationsError::StreamNotFound {
                    stream: stream.clone(),
                })?;
            if let Err(withheld) = self.pipeline.members().gate(&declared.audience, user).await {
                tracing::debug!(stream = %stream, user_id = %user, reason = %withheld, "Subscription refused");
                return Err(NotificationsError::NotInAudience {
                    stream: stream.clone(),
                });
            }
        }
        let source = ChangeSource::Console(user.clone()).to_string();
        let id = self
            .repos
            .subscriptions
            .upsert_pooled(&decl, &source, user)
            .await?
            .ok_or_else(|| NotificationsError::DeclaredByConfig {
                entity: "notification_subscription",
                key: target_key(&decl),
            })?;
        Ok(SubscriptionView { id, decl, source })
    }

    pub async fn unsubscribe(
        &self,
        user: &UserId,
        id: &NotificationSubscriptionId,
    ) -> NotificationsResult<()> {
        if self.repos.subscriptions.delete_owned(id, user).await? {
            Ok(())
        } else {
            Err(NotificationsError::SubscriptionNotFound { id: id.clone() })
        }
    }
}

fn target_key(decl: &SubscriptionDecl) -> String {
    match &decl.target {
        SubscriptionTarget::Stream(stream) => format!("{}|stream:{stream}", decl.subject),
        SubscriptionTarget::Resource(r) => format!("{}|resource:{}:{}", decl.subject, r.kind, r.id),
    }
}
