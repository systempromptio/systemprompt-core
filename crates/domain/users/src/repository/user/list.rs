use systemprompt_identifiers::UserId;

use crate::error::Result;
use crate::models::{User, UserActivity, UserRole, UserStats, UserStatus, UserWithSessions};
use crate::repository::{UserRepository, MAX_PAGE_SIZE};

impl UserRepository {
    pub async fn get_with_sessions(&self, user_id: &UserId) -> Result<Option<UserWithSessions>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let row = sqlx::query_as!(
            UserWithSessions,
            r#"
            SELECT
                u.id, u.name, u.email, u.full_name, u.status, u.roles, u.created_at,
                COUNT(s.session_id) FILTER (WHERE s.ended_at IS NULL) as "active_sessions!",
                MAX(s.last_activity_at) as last_session_at
            FROM users u
            LEFT JOIN user_sessions s ON s.user_id = u.id
            WHERE u.id = $1 AND u.status != $2
            GROUP BY u.id
            "#,
            user_id.as_str(),
            deleted_status
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn get_activity(&self, user_id: &UserId) -> Result<UserActivity> {
        let row = sqlx::query_as!(
            UserActivity,
            r#"
            SELECT
                u.id as user_id,
                MAX(s.last_activity_at) as last_active,
                COUNT(DISTINCT s.session_id) as "session_count!",
                COUNT(DISTINCT t.task_id) as "task_count!",
                0::bigint as "message_count!"
            FROM users u
            LEFT JOIN user_sessions s ON s.user_id = u.id
            LEFT JOIN agent_tasks t ON t.user_id = u.id
            WHERE u.id = $1
            GROUP BY u.id
            "#,
            user_id.as_str()
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<User>> {
        let safe_limit = limit.min(MAX_PAGE_SIZE);
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE status != $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            deleted_status,
            safe_limit,
            offset
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn list_all(&self) -> Result<Vec<User>> {
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE status != $1
            ORDER BY created_at DESC
            "#,
            deleted_status
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        let safe_limit = limit.min(MAX_PAGE_SIZE);
        let pattern = format!("%{}%", query);
        let deleted_status = UserStatus::Deleted.as_str();
        let rows = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE status != $1
              AND (name ILIKE $2 OR email ILIKE $2 OR full_name ILIKE $2)
            ORDER BY
                CASE WHEN name ILIKE $2 THEN 0 ELSE 1 END,
                created_at DESC
            LIMIT $3
            "#,
            deleted_status,
            pattern,
            safe_limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn count(&self) -> Result<i64> {
        let deleted_status = UserStatus::Deleted.as_str();
        let result = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!" FROM users WHERE status != $1"#,
            deleted_status
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }

    pub async fn count_by_status(&self) -> Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct StatusCount {
            status: String,
            cnt: i64,
        }

        let deleted_status = UserStatus::Deleted.as_str();
        let rows: Vec<StatusCount> = sqlx::query_as(
            r"
            SELECT status, COUNT(*)::bigint as cnt
            FROM users
            WHERE status != $1
            GROUP BY status
            ORDER BY cnt DESC
            ",
        )
        .bind(deleted_status)
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.status, r.cnt)).collect())
    }

    pub async fn count_by_role(&self) -> Result<Vec<(String, i64)>> {
        #[derive(sqlx::FromRow)]
        struct RoleCount {
            role: String,
            cnt: i64,
        }

        let deleted_status = UserStatus::Deleted.as_str();
        let rows: Vec<RoleCount> = sqlx::query_as(
            r"
            SELECT role, COUNT(*)::bigint as cnt
            FROM users, UNNEST(roles) as role
            WHERE status != $1
            GROUP BY role
            ORDER BY cnt DESC
            ",
        )
        .bind(deleted_status)
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| (r.role, r.cnt)).collect())
    }

    pub async fn get_stats(&self) -> Result<UserStats> {
        #[derive(sqlx::FromRow)]
        struct StatsRow {
            total: i64,
            created_24h: i64,
            created_7d: i64,
            created_30d: i64,
            active: i64,
            suspended: i64,
            admins: i64,
            anonymous: i64,
            bots: i64,
            oldest_user: Option<chrono::DateTime<chrono::Utc>>,
            newest_user: Option<chrono::DateTime<chrono::Utc>>,
        }

        let deleted_status = UserStatus::Deleted.as_str();
        let row: StatsRow = sqlx::query_as(
            r"
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
            ",
        )
        .bind(deleted_status)
        .fetch_one(&*self.pool)
        .await?;

        Ok(UserStats {
            total: row.total,
            created_24h: row.created_24h,
            created_7d: row.created_7d,
            created_30d: row.created_30d,
            active: row.active,
            suspended: row.suspended,
            admins: row.admins,
            anonymous: row.anonymous,
            bots: row.bots,
            oldest_user: row.oldest_user,
            newest_user: row.newest_user,
        })
    }

    pub async fn bulk_update_status(&self, user_ids: &[UserId], new_status: &str) -> Result<u64> {
        let ids: Vec<String> = user_ids.iter().map(ToString::to_string).collect();
        let result = sqlx::query!(
            r#"
            UPDATE users
            SET status = $1, updated_at = NOW()
            WHERE id = ANY($2)
            "#,
            new_status,
            &ids[..]
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn bulk_delete(&self, user_ids: &[UserId]) -> Result<u64> {
        let deleted_status = UserStatus::Deleted.as_str();
        let ids: Vec<String> = user_ids.iter().map(ToString::to_string).collect();
        let result = sqlx::query!(
            r#"
            UPDATE users
            SET status = $1, updated_at = NOW()
            WHERE id = ANY($2)
            "#,
            deleted_status,
            &ids[..]
        )
        .execute(&*self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn list_by_filter(
        &self,
        status: Option<&str>,
        role: Option<&str>,
        older_than_days: Option<i64>,
        limit: i64,
    ) -> Result<Vec<User>> {
        let safe_limit = limit.min(MAX_PAGE_SIZE);
        let deleted_status = UserStatus::Deleted.as_str();

        let rows = sqlx::query_as!(
            User,
            r#"
            SELECT id, name, email, full_name, display_name, status, email_verified,
                   roles, avatar_url, is_bot, is_scanner, created_at, updated_at
            FROM users
            WHERE status != $1
              AND ($2::text IS NULL OR status = $2)
              AND ($3::text IS NULL OR $3 = ANY(roles))
              AND ($4::bigint IS NULL OR created_at < NOW() - make_interval(days => $4::int))
            ORDER BY created_at DESC
            LIMIT $5
            "#,
            deleted_status,
            status,
            role,
            older_than_days,
            safe_limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn is_temporary_anonymous(&self, id: &UserId) -> Result<bool> {
        let anonymous_role = UserRole::Anonymous.as_str();
        let result = sqlx::query_scalar!(
            r#"
            SELECT $1 = ANY(roles) as "is_anonymous!"
            FROM users
            WHERE id = $2
            "#,
            anonymous_role,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(result.unwrap_or(false))
    }

    pub async fn list_non_anonymous_with_sessions(
        &self,
        limit: i64,
    ) -> Result<Vec<UserWithSessions>> {
        let safe_limit = limit.min(MAX_PAGE_SIZE);
        let deleted_status = UserStatus::Deleted.as_str();
        let anonymous_role = UserRole::Anonymous.as_str();
        let rows = sqlx::query_as!(
            UserWithSessions,
            r#"
            SELECT
                u.id, u.name, u.email, u.full_name, u.status, u.roles, u.created_at,
                COUNT(s.session_id) FILTER (WHERE s.ended_at IS NULL) as "active_sessions!",
                MAX(s.last_activity_at) as last_session_at
            FROM users u
            LEFT JOIN user_sessions s ON s.user_id = u.id
            WHERE u.status != $1
              AND NOT ($2 = ANY(u.roles))
            GROUP BY u.id
            ORDER BY last_session_at DESC NULLS LAST
            LIMIT $3
            "#,
            deleted_status,
            anonymous_role,
            safe_limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }
}
