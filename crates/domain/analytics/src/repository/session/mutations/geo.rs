//! Geo backfill writes against `user_sessions`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use sqlx::PgPool;

pub(crate) async fn backfill_session_geo(
    pool: &PgPool,
    geoip_reader: Option<&crate::GeoIpReader>,
    batch_size: i64,
) -> Result<u64> {
    let mut updated = 0u64;
    let mut last_session_id = String::new();

    loop {
        let rows = sqlx::query!(
            r#"
            SELECT session_id, ip_address as "ip_address!"
            FROM user_sessions
            WHERE country IS NULL AND ip_address IS NOT NULL AND session_id > $1
            ORDER BY session_id
            LIMIT $2
            "#,
            last_session_id,
            batch_size
        )
        .fetch_all(pool)
        .await?;

        let Some(last) = rows.last() else {
            break;
        };
        last_session_id = last.session_id.clone();

        for row in &rows {
            let Some((country, region, city)) =
                crate::services::extractor::geoip::lookup_geoip(&row.ip_address, geoip_reader)
            else {
                continue;
            };
            let result = sqlx::query!(
                r#"
                UPDATE user_sessions
                SET country = $2, region = $3, city = $4
                WHERE session_id = $1 AND country IS NULL
                "#,
                row.session_id,
                country,
                region,
                city
            )
            .execute(pool)
            .await?;
            updated += result.rows_affected();
        }
    }

    Ok(updated)
}

pub(crate) async fn count_sessions_missing_geo(pool: &PgPool) -> Result<i64> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM user_sessions
        WHERE country IS NULL AND ip_address IS NOT NULL
        "#
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
