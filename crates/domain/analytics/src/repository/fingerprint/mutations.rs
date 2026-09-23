//! Fingerprint-reputation state changes for `FingerprintRepository`.
//!
//! Upserts a fingerprint on each session, counts its requests and raises
//! abuse flags. All writes go to the write pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use systemprompt_identifiers::UserId;

use super::FingerprintRepository;
use crate::models::{FingerprintReputation, FlagReason};

impl FingerprintRepository {
    pub async fn upsert_fingerprint(
        &self,
        fingerprint_hash: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        user_id: Option<&UserId>,
    ) -> Result<FingerprintReputation> {
        let user_ids = user_id.map_or_else(Vec::new, |u| vec![u.as_str().to_owned()]);

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
        .fetch_one(&*self.write_pool)
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
        .execute(&*self.write_pool)
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
        .execute(&*self.write_pool)
        .await?;

        Ok(())
    }
}
