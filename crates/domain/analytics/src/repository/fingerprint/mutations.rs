use anyhow::Result;

use super::FingerprintRepository;
use crate::models::{FingerprintReputation, FlagReason};

impl FingerprintRepository {
    pub async fn upsert_fingerprint(
        &self,
        fingerprint_hash: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        user_id: Option<&str>,
    ) -> Result<FingerprintReputation> {
        let user_ids = user_id.map_or_else(Vec::new, |u| vec![u.to_string()]);

        let row = sqlx::query_as!(
            FingerprintReputation,
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash, last_ip_address, last_user_agent,
                associated_user_ids, total_session_count
            )
            VALUES ($1, $2, $3, $4, 1)
            ON CONFLICT (fingerprint_hash) DO UPDATE SET
                last_seen_at = CURRENT_TIMESTAMP,
                last_ip_address = COALESCE($2, fingerprint_reputation.last_ip_address),
                last_user_agent = COALESCE($3, fingerprint_reputation.last_user_agent),
                total_session_count = fingerprint_reputation.total_session_count + 1,
                associated_user_ids = CASE
                    WHEN array_length($4, 1) > 0 AND NOT ($4[1] = ANY(fingerprint_reputation.associated_user_ids))
                    THEN array_cat(fingerprint_reputation.associated_user_ids, $4)
                    ELSE fingerprint_reputation.associated_user_ids
                END,
                updated_at = CURRENT_TIMESTAMP
            RETURNING
                fingerprint_hash,
                first_seen_at,
                last_seen_at,
                total_session_count,
                active_session_count,
                total_request_count,
                requests_last_hour,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                is_flagged,
                flag_reason,
                flagged_at,
                reputation_score,
                abuse_incidents,
                last_abuse_at,
                last_ip_address,
                last_user_agent,
                associated_user_ids,
                updated_at
            "#,
            fingerprint_hash,
            ip_address,
            user_agent,
            &user_ids[..],
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn flag_fingerprint(
        &self,
        fingerprint_hash: &str,
        reason: FlagReason,
        new_score: i32,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE fingerprint_reputation
            SET is_flagged = TRUE,
                flag_reason = $2,
                flagged_at = CURRENT_TIMESTAMP,
                reputation_score = $3,
                abuse_incidents = abuse_incidents + 1,
                last_abuse_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
            reason.as_str(),
            new_score,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_velocity_metrics(
        &self,
        fingerprint_hash: &str,
        requests_last_hour: i32,
        peak_requests_per_minute: f32,
        sustained_high_velocity_minutes: i32,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE fingerprint_reputation
            SET requests_last_hour = $2,
                peak_requests_per_minute = $3,
                sustained_high_velocity_minutes = $4,
                total_request_count = total_request_count + 1,
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
            requests_last_hour,
            peak_requests_per_minute,
            sustained_high_velocity_minutes,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_active_session_count(
        &self,
        fingerprint_hash: &str,
        active_count: i32,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE fingerprint_reputation
            SET active_session_count = $2,
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
            active_count,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn increment_request_count(&self, fingerprint_hash: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE fingerprint_reputation
            SET total_request_count = total_request_count + 1,
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn clear_flag(&self, fingerprint_hash: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE fingerprint_reputation
            SET is_flagged = FALSE,
                flag_reason = NULL,
                flagged_at = NULL,
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn adjust_reputation_score(&self, fingerprint_hash: &str, delta: i32) -> Result<i32> {
        let row = sqlx::query_scalar!(
            r#"
            UPDATE fingerprint_reputation
            SET reputation_score = GREATEST(0, LEAST(100, reputation_score + $2)),
                updated_at = CURRENT_TIMESTAMP
            WHERE fingerprint_hash = $1
            RETURNING reputation_score as "reputation_score!"
            "#,
            fingerprint_hash,
            delta,
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(row)
    }
}
