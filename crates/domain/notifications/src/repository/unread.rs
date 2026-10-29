//! Unread counts, capped: each count reads at most `UNREAD_CAP + 1`
//! rows, and a count above the cap reports the cap with `capped` set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;

use super::NotificationRepository;

pub const UNREAD_CAP: i64 = 1000;

impl NotificationRepository {
    pub async fn unread_counts(
        &self,
        recipients: &[UserId],
    ) -> Result<Vec<(UserId, i64, bool)>, RepositoryError> {
        let mut conn = self.pools.read.acquire().await?;
        Self::unread_counts_on(&mut conn, recipients).await
    }

    pub async fn unread_counts_on(
        conn: &mut PgConnection,
        recipients: &[UserId],
    ) -> Result<Vec<(UserId, i64, bool)>, RepositoryError> {
        if recipients.is_empty() {
            return Ok(Vec::new());
        }
        let users: Vec<String> = recipients.iter().map(|u| u.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT u.id AS "user!: UserId",
                      (SELECT COUNT(*) FROM (SELECT 1 FROM notifications n
                                             WHERE n.recipient_user_id = u.id
                                               AND n.read_at IS NULL
                                             LIMIT $2) capped) AS "count!"
               FROM UNNEST($1::text[]) AS u(id)"#,
            &users,
            UNREAD_CAP + 1,
        )
        .fetch_all(&mut *conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.user, row.count.min(UNREAD_CAP), row.count > UNREAD_CAP))
            .collect())
    }

    pub async fn unread_count(&self, recipient: &UserId) -> Result<(i64, bool), RepositoryError> {
        Ok(self
            .unread_counts(std::slice::from_ref(recipient))
            .await?
            .into_iter()
            .next()
            .map_or((0, false), |(_, count, capped)| (count, capped)))
    }
}
