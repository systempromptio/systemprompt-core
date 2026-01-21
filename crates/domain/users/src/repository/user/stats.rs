use chrono::{DateTime, Utc};

use crate::error::Result;
use crate::models::{UserStats, UserStatus};
use crate::repository::UserRepository;

#[derive(sqlx::FromRow)]
struct StatusCount {
    status: Option<String>,
    cnt: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct RoleCount {
    role: Option<String>,
    cnt: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct StatsRow {
    total: Option<i64>,
    created_24h: Option<i64>,
    created_7d: Option<i64>,
    created_30d: Option<i64>,
    active: Option<i64>,
    suspended: Option<i64>,
    admins: Option<i64>,
    anonymous: Option<i64>,
    bots: Option<i64>,
    oldest_user: Option<DateTime<Utc>>,
    newest_user: Option<DateTime<Utc>>,
}

impl UserRepository {
    pub async fn count_by_status(&self) -> Result<Vec<(String, i64)>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            StatusCount,
            r#"
            SELECT status, COUNT(*)::bigint as cnt
            FROM users
            WHERE status != $1
            GROUP BY status
            ORDER BY cnt DESC
            "#,
            deleted_status
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| Some((r.status?, r.cnt?)))
            .collect())
    }

    pub async fn count_by_role(&self) -> Result<Vec<(String, i64)>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            RoleCount,
            r#"
            SELECT role, COUNT(*)::bigint as cnt
            FROM users, UNNEST(roles) as role
            WHERE status != $1
            GROUP BY role
            ORDER BY cnt DESC
            "#,
            deleted_status
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| Some((r.role?, r.cnt?)))
            .collect())
    }

    pub async fn get_stats(&self) -> Result<UserStats> {
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            StatsRow,
            r#"
            SELECT
                COUNT(*)::bigint as total,
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '24 hours')::bigint as created_24h,
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '7 days')::bigint as created_7d,
                COUNT(*) FILTER (WHERE created_at >= NOW() - INTERVAL '30 days')::bigint as created_30d,
                COUNT(*) FILTER (WHERE status = 'active')::bigint as active,
                COUNT(*) FILTER (WHERE status = 'suspended')::bigint as suspended,
                COUNT(*) FILTER (WHERE 'admin' = ANY(roles))::bigint as admins,
                COUNT(*) FILTER (WHERE 'anonymous' = ANY(roles))::bigint as anonymous,
                COUNT(*) FILTER (WHERE is_bot = true)::bigint as bots,
                MIN(created_at) as oldest_user,
                MAX(created_at) as newest_user
            FROM users
            WHERE status != $1
            "#,
            deleted_status
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(UserStats {
            total: row.total.unwrap_or(0),
            created_24h: row.created_24h.unwrap_or(0),
            created_7d: row.created_7d.unwrap_or(0),
            created_30d: row.created_30d.unwrap_or(0),
            active: row.active.unwrap_or(0),
            suspended: row.suspended.unwrap_or(0),
            admins: row.admins.unwrap_or(0),
            anonymous: row.anonymous.unwrap_or(0),
            bots: row.bots.unwrap_or(0),
            oldest_user: row.oldest_user,
            newest_user: row.newest_user,
        })
    }
}
