//! Session geolocation persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::SessionRepository;
use crate::Result;
use sqlx::PgPool;
use systemprompt_identifiers::SessionId;

pub(super) async fn count_sessions_missing_geo(pool: &PgPool) -> Result<i64> {
    Ok(sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM user_sessions
        WHERE country IS NULL AND ip_address IS NOT NULL
        "#
    )
    .fetch_one(pool)
    .await?)
}

impl SessionRepository {
    pub(super) async fn missing_geo(
        &self,
        after: Option<&SessionId>,
        limit: i64,
    ) -> Result<Vec<(SessionId, String)>> {
        let rows = sqlx::query!(
            r#"
            SELECT session_id, ip_address as "ip_address!"
            FROM user_sessions
            WHERE country IS NULL AND ip_address IS NOT NULL AND session_id > $1
            ORDER BY session_id
            LIMIT $2
            "#,
            after.map_or("", SessionId::as_str),
            limit
        )
        .fetch_all(&*self.write_pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (SessionId::new(r.session_id), r.ip_address))
            .collect())
    }

    pub(super) async fn set_geo(
        &self,
        session_id: &SessionId,
        country: Option<&str>,
        region: Option<&str>,
        city: Option<&str>,
    ) -> Result<u64> {
        Ok(sqlx::query!(
            r#"
                UPDATE user_sessions
                SET country = $2, region = $3, city = $4
                WHERE session_id = $1 AND country IS NULL
                "#,
            session_id.as_str(),
            country,
            region,
            city
        )
        .execute(&*self.write_pool)
        .await?
        .rows_affected())
    }
}
