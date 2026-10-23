//! The gateway session bound to each API key (`api_key_sessions`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ApiKeyId, SessionId};

use crate::error::Result;
use crate::repository::UserRepository;

impl UserRepository {
    pub async fn find_api_key_session(&self, api_key_id: &ApiKeyId) -> Result<Option<SessionId>> {
        let row = sqlx::query_scalar!(
            "SELECT session_id FROM api_key_sessions WHERE api_key_id = $1",
            api_key_id.as_str(),
        )
        .fetch_optional(&*self.write_pool)
        .await?;
        Ok(row.map(SessionId::new))
    }

    pub async fn set_api_key_session(
        &self,
        api_key_id: &ApiKeyId,
        session_id: &SessionId,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO api_key_sessions (api_key_id, session_id)
            VALUES ($1, $2)
            ON CONFLICT (api_key_id)
            DO UPDATE SET session_id = EXCLUDED.session_id, bound_at = CURRENT_TIMESTAMP
            "#,
            api_key_id.as_str(),
            session_id.as_str(),
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
