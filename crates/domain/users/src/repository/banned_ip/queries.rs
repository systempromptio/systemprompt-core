use anyhow::Result;

use super::types::{BanDuration, BanIpParams, BanIpWithMetadataParams, BannedIp};
use super::BannedIpRepository;

impl BannedIpRepository {
    pub async fn is_banned(&self, ip_address: &str) -> Result<bool> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM banned_ips
                WHERE ip_address = $1
                  AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            ) as "exists!"
            "#,
            ip_address
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }

    pub async fn find_ban(&self, ip_address: &str) -> Result<Option<BannedIp>> {
        let row = sqlx::query_as!(
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
            WHERE ip_address = $1
              AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)
            "#,
            ip_address
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn ban_ip(&self, params: BanIpParams<'_>) -> Result<()> {
        let expires_at = params.duration.to_expiry();
        let is_permanent = matches!(params.duration, BanDuration::Permanent);

        sqlx::query!(
            r#"
            INSERT INTO banned_ips (
                ip_address, reason, expires_at, is_permanent,
                source_fingerprint, ban_source
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (ip_address) DO UPDATE SET
                reason = $2,
                expires_at = CASE
                    WHEN banned_ips.is_permanent THEN banned_ips.expires_at
                    ELSE COALESCE($3, banned_ips.expires_at)
                END,
                ban_count = banned_ips.ban_count + 1,
                is_permanent = banned_ips.is_permanent OR $4,
                source_fingerprint = COALESCE($5, banned_ips.source_fingerprint),
                ban_source = $6
            "#,
            params.ip_address,
            params.reason,
            expires_at,
            is_permanent,
            params.source_fingerprint,
            params.ban_source
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn ban_ip_with_metadata(&self, params: BanIpWithMetadataParams<'_>) -> Result<()> {
        let expires_at = params.duration.to_expiry();
        let is_permanent = matches!(params.duration, BanDuration::Permanent);
        let session_ids: Option<Vec<String>> = params.session_id.map(|s| vec![s.to_string()]);

        sqlx::query!(
            r#"
            INSERT INTO banned_ips (
                ip_address, reason, expires_at, is_permanent,
                source_fingerprint, ban_source, last_offense_path,
                last_user_agent, associated_session_ids
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (ip_address) DO UPDATE SET
                reason = $2,
                expires_at = CASE
                    WHEN banned_ips.is_permanent THEN banned_ips.expires_at
                    ELSE COALESCE($3, banned_ips.expires_at)
                END,
                ban_count = banned_ips.ban_count + 1,
                is_permanent = banned_ips.is_permanent OR $4,
                source_fingerprint = COALESCE($5, banned_ips.source_fingerprint),
                ban_source = $6,
                last_offense_path = COALESCE($7, banned_ips.last_offense_path),
                last_user_agent = COALESCE($8, banned_ips.last_user_agent),
                associated_session_ids = CASE
                    WHEN $9::TEXT[] IS NOT NULL
                    THEN array_cat(COALESCE(banned_ips.associated_session_ids, '{}'::TEXT[]), $9)
                    ELSE banned_ips.associated_session_ids
                END
            "#,
            params.ip_address,
            params.reason,
            expires_at,
            is_permanent,
            params.source_fingerprint,
            params.ban_source,
            params.offense_path,
            params.user_agent,
            session_ids.as_deref()
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }

    pub async fn unban_ip(&self, ip_address: &str) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            DELETE FROM banned_ips
            WHERE ip_address = $1
            "#,
            ip_address
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn cleanup_expired(&self) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM banned_ips
            WHERE expires_at IS NOT NULL
              AND expires_at < CURRENT_TIMESTAMP
              AND NOT is_permanent
            "#
        )
        .execute(&*self.write_pool)
        .await?;

        Ok(result.rows_affected())
    }
}
