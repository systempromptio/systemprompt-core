//! Fingerprint-reputation read queries for `FingerprintRepository`.
//!
//! Looks up a fingerprint by hash, counts and finds reusable active sessions,
//! and lists recent or high-risk fingerprints for the abuse-analysis job. All
//! reads go to the read pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;

use super::FingerprintRepository;
use crate::models::FingerprintReputation;
use systemprompt_identifiers::SessionId;

impl FingerprintRepository {
    pub async fn get_by_hash(
        &self,
        fingerprint_hash: &str,
    ) -> Result<Option<FingerprintReputation>> {
        let row = sqlx::query_as!(
            FingerprintReputation,
            r#"
            SELECT
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
            FROM fingerprint_reputation
            WHERE fingerprint_hash = $1
            "#,
            fingerprint_hash,
        )
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row)
    }

    pub async fn count_active_sessions(&self, fingerprint_hash: &str) -> Result<i32> {
        self.sessions
            .count_active_fingerprint_sessions(fingerprint_hash)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn find_reusable_session(&self, fingerprint_hash: &str) -> Result<Option<SessionId>> {
        self.sessions
            .find_reusable_fingerprint_session(fingerprint_hash)
            .await
            .map_err(crate::AnalyticsError::from)
    }

    pub async fn get_fingerprints_for_analysis(&self) -> Result<Vec<FingerprintReputation>> {
        let rows = sqlx::query_as!(
            FingerprintReputation,
            r#"
            SELECT
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
            FROM fingerprint_reputation
            WHERE last_seen_at > CURRENT_TIMESTAMP - INTERVAL '1 hour'
            ORDER BY total_request_count DESC
            LIMIT 1000
            "#,
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }

    pub async fn get_high_risk_fingerprints(
        &self,
        limit: i64,
    ) -> Result<Vec<FingerprintReputation>> {
        let rows = sqlx::query_as!(
            FingerprintReputation,
            r#"
            SELECT
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
            FROM fingerprint_reputation
            WHERE is_flagged = TRUE
               OR reputation_score < 30
               OR abuse_incidents >= 3
            ORDER BY reputation_score ASC, abuse_incidents DESC
            LIMIT $1
            "#,
            limit,
        )
        .fetch_all(&*self.pool)
        .await?;

        Ok(rows)
    }
}
