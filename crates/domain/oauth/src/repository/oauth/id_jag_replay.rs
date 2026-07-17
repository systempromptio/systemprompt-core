//! Single-use replay store for EMA ID-JAGs. Cross-instance authoritative, so
//! consumption is an atomic `INSERT ... ON CONFLICT DO NOTHING` keyed on `jti`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::OAuthRepository;
use crate::error::OauthResult;
use chrono::{DateTime, Utc};

impl OAuthRepository {
    /// Records `jti` and returns `true` only on its first presentation; `false`
    /// signals a replay that must be rejected.
    pub async fn consume_id_jag_jti(
        &self,
        jti: &str,
        expires_at: DateTime<Utc>,
    ) -> OauthResult<bool> {
        let result = sqlx::query!(
            "INSERT INTO id_jag_replay (jti, expires_at)
             VALUES ($1, $2)
             ON CONFLICT (jti) DO NOTHING",
            jti,
            expires_at,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
