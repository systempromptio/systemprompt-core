//! Single-use bridge exchange code persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::OauthResult as Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use crate::repository::OAuthRepository;

#[derive(Debug)]
pub struct CreateExchangeCodeParams<'a> {
    pub code_hash: &'a str,
    pub user_id: &'a UserId,
    pub expires_at: DateTime<Utc>,
}

impl OAuthRepository {
    pub async fn create_bridge_exchange_code(
        &self,
        params: CreateExchangeCodeParams<'_>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO bridge_exchange_codes (code_hash, user_id, expires_at)
            VALUES ($1, $2, $3)
            "#,
            params.code_hash,
            params.user_id.as_str(),
            params.expires_at,
        )
        .execute(self.write_pool_ref())
        .await?;
        Ok(())
    }

    pub async fn consume_bridge_exchange_code(&self, code_hash: &str) -> Result<Option<UserId>> {
        let row = sqlx::query!(
            r#"
            UPDATE bridge_exchange_codes
            SET consumed_at = CURRENT_TIMESTAMP
            WHERE code_hash = $1
              AND consumed_at IS NULL
              AND expires_at > CURRENT_TIMESTAMP
            RETURNING user_id
            "#,
            code_hash,
        )
        .fetch_optional(self.write_pool_ref())
        .await?;
        Ok(row.map(|r| UserId::new(r.user_id)))
    }
}
