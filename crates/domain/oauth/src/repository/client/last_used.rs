//! Client last-use stamping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::ClientRepository;
use crate::error::OauthResult as Result;
use chrono::Utc;
use systemprompt_identifiers::ClientId;

impl ClientRepository {
    pub async fn update_last_used(&self, client_id: &ClientId, timestamp: i64) -> Result<()> {
        let dt = chrono::DateTime::<Utc>::from_timestamp(timestamp, 0)
            .ok_or_else(|| crate::error::OauthError::Internal("Invalid timestamp".to_owned()))?;
        let client_id_str = client_id.as_str();
        sqlx::query!(
            "UPDATE oauth_clients SET last_used_at = $1 WHERE client_id = $2",
            dt,
            client_id_str
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
