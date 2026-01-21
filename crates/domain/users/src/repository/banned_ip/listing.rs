use anyhow::Result;

use super::types::BannedIp;
use super::BannedIpRepository;

impl BannedIpRepository {
    pub async fn list_active_bans(&self, limit: i64) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as!(
            BannedIp,
            r#"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP
            ORDER BY banned_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn list_bans_by_source(&self, ban_source: &str, limit: i64) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as!(
            BannedIp,
            r#"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE ban_source = $1
              AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            ORDER BY banned_at DESC
            LIMIT $2
            "#,
            ban_source,
            limit
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn list_bans_by_fingerprint(&self, fingerprint: &str) -> Result<Vec<BannedIp>> {
        let bans = sqlx::query_as!(
            BannedIp,
            r#"
            SELECT
                ip_address,
                reason,
                banned_at,
                expires_at,
                ban_count,
                last_offense_path,
                last_user_agent,
                is_permanent,
                source_fingerprint,
                ban_source,
                associated_session_ids
            FROM banned_ips
            WHERE source_fingerprint = $1
            ORDER BY banned_at DESC
            "#,
            fingerprint
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(bans)
    }

    pub async fn count_active_bans(&self) -> Result<i64> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*)::BIGINT as "count!"
            FROM banned_ips
            WHERE expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP
            "#
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }
}
