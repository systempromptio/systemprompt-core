//! Writes to `notification_subscriptions`. An upsert never changes a row's
//! source: a write whose origin (declared `code:` or not) differs from the
//! stored row's leaves it untouched and answers `None`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgExecutor;
use systemprompt_identifiers::{NotificationSubscriptionId, UserId};
use systemprompt_models::notifications::{Attention, SubscriptionDecl, SubscriptionTarget};
use systemprompt_provider_contracts::ChangeSource;
use systemprompt_traits::RepositoryError;

use super::NotificationSubscriptionRepository;

impl NotificationSubscriptionRepository {
    pub async fn upsert(
        executor: impl PgExecutor<'_>,
        decl: &SubscriptionDecl,
        source: &ChangeSource,
        created_by: Option<&UserId>,
    ) -> Result<Option<NotificationSubscriptionId>, RepositoryError> {
        let subject = decl.subject.to_string();
        let ceiling = decl.attention_ceiling.map(Attention::as_str);
        let quiet = decl
            .quiet_hours
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;
        let digest = decl.digest.as_ref().map(serde_json::to_value).transpose()?;
        let created_by = created_by.map(UserId::as_str);
        let source = source.to_string();
        let id = match &decl.target {
            SubscriptionTarget::Stream(stream) => {
                sqlx::query_scalar!(
                    r#"INSERT INTO notification_subscriptions
                           (subject, target_stream, attention_ceiling, quiet_hours, digest,
                            source, created_by)
                       VALUES ($1, $2, $3, $4, $5, $6, $7)
                       ON CONFLICT (subject, target_stream) WHERE target_stream IS NOT NULL
                       DO UPDATE SET attention_ceiling = EXCLUDED.attention_ceiling,
                                     quiet_hours = EXCLUDED.quiet_hours,
                                     digest = EXCLUDED.digest
                       WHERE (notification_subscriptions.source LIKE 'code:%')
                           = (EXCLUDED.source LIKE 'code:%')
                       RETURNING id::text AS "id!: NotificationSubscriptionId""#,
                    subject,
                    stream.as_str(),
                    ceiling,
                    quiet,
                    digest,
                    source,
                    created_by,
                )
                .fetch_optional(executor)
                .await?
            },
            SubscriptionTarget::Resource(resource) => {
                sqlx::query_scalar!(
                    r#"INSERT INTO notification_subscriptions
                           (subject, target_kind, target_id, attention_ceiling, quiet_hours,
                            digest, source, created_by)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                       ON CONFLICT (subject, target_kind, target_id) WHERE target_kind IS NOT NULL
                       DO UPDATE SET attention_ceiling = EXCLUDED.attention_ceiling,
                                     quiet_hours = EXCLUDED.quiet_hours,
                                     digest = EXCLUDED.digest
                       WHERE (notification_subscriptions.source LIKE 'code:%')
                           = (EXCLUDED.source LIKE 'code:%')
                       RETURNING id::text AS "id!: NotificationSubscriptionId""#,
                    subject,
                    resource.kind,
                    resource.id,
                    ceiling,
                    quiet,
                    digest,
                    source,
                    created_by,
                )
                .fetch_optional(executor)
                .await?
            },
        };
        Ok(id)
    }

    pub async fn upsert_pooled(
        &self,
        decl: &SubscriptionDecl,
        source: &ChangeSource,
        created_by: &UserId,
    ) -> Result<Option<NotificationSubscriptionId>, RepositoryError> {
        Self::upsert(&*self.pools.write, decl, source, Some(created_by)).await
    }

    pub async fn delete_owned(
        &self,
        id: &NotificationSubscriptionId,
        owner: &UserId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "DELETE FROM notification_subscriptions
             WHERE id = $1::text::uuid AND subject_user_id = $2",
            id.as_str(),
            owner.as_str(),
        )
        .execute(&*self.pools.write)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_declared(
        executor: impl PgExecutor<'_>,
        decl: &SubscriptionDecl,
    ) -> Result<(), RepositoryError> {
        let subject = decl.subject.to_string();
        let (stream, kind, id) = match &decl.target {
            SubscriptionTarget::Stream(stream) => (Some(stream.as_str()), None, None),
            SubscriptionTarget::Resource(r) => (None, Some(r.kind.as_str()), Some(r.id.as_str())),
        };
        sqlx::query!(
            "DELETE FROM notification_subscriptions
             WHERE subject = $1 AND source LIKE 'code:%'
               AND target_stream IS NOT DISTINCT FROM $2
               AND target_kind IS NOT DISTINCT FROM $3
               AND target_id IS NOT DISTINCT FROM $4",
            subject,
            stream,
            kind,
            id,
        )
        .execute(executor)
        .await?;
        Ok(())
    }
}
