//! [`NotificationSubscriptionRepository`]: subscriptions of an audience to a
//! stream or a resource. Declared rows carry a `code:` source and belong to
//! the config plane; a user's own rows carry `console:<user>`. A row's source
//! never changes after it is written (see the writes in
//! `subscription_writes`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgExecutor;
use sqlx::types::Json;
use systemprompt_identifiers::{NotificationStreamId, NotificationSubscriptionId, UserId};
use systemprompt_models::notifications::{
    AudienceSubject, DigestWindow, QuietHours, SubjectRef, SubscriptionDecl, SubscriptionTarget,
};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{SubscriptionView, parse, parse_opt};

struct SubscriptionRow {
    id: NotificationSubscriptionId,
    subject: String,
    target_stream: Option<NotificationStreamId>,
    target_kind: Option<String>,
    target_id: Option<String>,
    attention_ceiling: Option<String>,
    quiet_hours: Option<Json<QuietHours>>,
    digest: Option<Json<DigestWindow>>,
    source: String,
}

impl TryFrom<SubscriptionRow> for SubscriptionView {
    type Error = RepositoryError;

    fn try_from(row: SubscriptionRow) -> Result<Self, Self::Error> {
        let target = match (row.target_stream, row.target_kind, row.target_id) {
            (Some(stream), None, None) => SubscriptionTarget::Stream(stream),
            (None, Some(kind), Some(id)) => SubscriptionTarget::Resource(SubjectRef { kind, id }),
            _ => {
                return Err(RepositoryError::invalid_data(
                    "notification_subscriptions.target",
                    "row names neither exactly one stream nor one resource",
                ));
            },
        };
        Ok(Self {
            id: row.id,
            decl: SubscriptionDecl {
                subject: parse("notification_subscriptions.subject", &row.subject)?,
                target,
                attention_ceiling: parse_opt(
                    "notification_subscriptions.attention_ceiling",
                    row.attention_ceiling.as_deref(),
                )?,
                quiet_hours: row.quiet_hours.map(|q| q.0),
                digest: row.digest.map(|d| d.0),
            },
            source: parse("notification_subscriptions.source", &row.source)?,
        })
    }
}

fn collect(rows: Vec<SubscriptionRow>) -> Result<Vec<SubscriptionView>, RepositoryError> {
    rows.into_iter().map(SubscriptionView::try_from).collect()
}

#[derive(Debug, Clone)]
pub struct NotificationSubscriptionRepository {
    pub(super) pools: Pools,
}

impl NotificationSubscriptionRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list_for_subjects(
        executor: impl PgExecutor<'_>,
        subjects: &[String],
    ) -> Result<Vec<SubscriptionView>, RepositoryError> {
        collect(
            sqlx::query_as!(
                SubscriptionRow,
                r#"SELECT id::text AS "id!: NotificationSubscriptionId", subject,
                          target_stream AS "target_stream: NotificationStreamId",
                          target_kind, target_id, attention_ceiling,
                          quiet_hours AS "quiet_hours: Json<QuietHours>",
                          digest AS "digest: Json<DigestWindow>", source
                   FROM notification_subscriptions WHERE subject = ANY($1)
                   ORDER BY created_at, id"#,
                subjects
            )
            .fetch_all(executor)
            .await?,
        )
    }

    pub async fn list_for_user(
        &self,
        user: &UserId,
    ) -> Result<Vec<SubscriptionView>, RepositoryError> {
        let subject = AudienceSubject::User(user.clone()).to_string();
        Self::list_for_subjects(&*self.pools.read, &[subject]).await
    }

    pub async fn list_for_stream(
        executor: impl PgExecutor<'_>,
        stream: &NotificationStreamId,
    ) -> Result<Vec<SubscriptionView>, RepositoryError> {
        collect(
            sqlx::query_as!(
                SubscriptionRow,
                r#"SELECT id::text AS "id!: NotificationSubscriptionId", subject,
                          target_stream AS "target_stream: NotificationStreamId",
                          target_kind, target_id, attention_ceiling,
                          quiet_hours AS "quiet_hours: Json<QuietHours>",
                          digest AS "digest: Json<DigestWindow>", source
                   FROM notification_subscriptions WHERE target_stream = $1
                   ORDER BY created_at, id"#,
                stream.as_str()
            )
            .fetch_all(executor)
            .await?,
        )
    }

    pub async fn list_for_resources(
        executor: impl PgExecutor<'_>,
        resources: &[SubjectRef],
    ) -> Result<Vec<SubscriptionView>, RepositoryError> {
        if resources.is_empty() {
            return Ok(Vec::new());
        }
        let kinds: Vec<String> = resources.iter().map(|r| r.kind.clone()).collect();
        let ids: Vec<String> = resources.iter().map(|r| r.id.clone()).collect();
        collect(
            sqlx::query_as!(
                SubscriptionRow,
                r#"SELECT s.id::text AS "id!: NotificationSubscriptionId", s.subject,
                          s.target_stream AS "target_stream: NotificationStreamId",
                          s.target_kind, s.target_id, s.attention_ceiling,
                          s.quiet_hours AS "quiet_hours: Json<QuietHours>",
                          s.digest AS "digest: Json<DigestWindow>", s.source
                   FROM notification_subscriptions s
                   JOIN UNNEST($1::text[], $2::text[]) AS r(kind, id)
                     ON s.target_kind = r.kind AND s.target_id = r.id
                   ORDER BY s.created_at, s.id"#,
                &kinds,
                &ids
            )
            .fetch_all(executor)
            .await?,
        )
    }

    pub async fn list_declared(
        executor: impl PgExecutor<'_>,
    ) -> Result<Vec<SubscriptionView>, RepositoryError> {
        collect(
            sqlx::query_as!(
                SubscriptionRow,
                r#"SELECT id::text AS "id!: NotificationSubscriptionId", subject,
                          target_stream AS "target_stream: NotificationStreamId",
                          target_kind, target_id, attention_ceiling,
                          quiet_hours AS "quiet_hours: Json<QuietHours>",
                          digest AS "digest: Json<DigestWindow>", source
                   FROM notification_subscriptions WHERE source LIKE 'code:%'
                   ORDER BY created_at, id"#
            )
            .fetch_all(executor)
            .await?,
        )
    }
}
