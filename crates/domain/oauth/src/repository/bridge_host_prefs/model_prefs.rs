//! The `bridge_user_host_model_prefs` half of [`BridgeHostPrefsRepository`]:
//! the per-host wire-protocol filter and its tag migration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_traits::{BridgeHostFilterMigration, RepositoryError};

use super::{BridgeHostPrefsRepository, known_host};
use crate::error::OauthResult;

impl BridgeHostPrefsRepository {
    pub async fn update_model_protocol_tag(
        &self,
        from: &str,
        to: &str,
    ) -> Result<u64, RepositoryError> {
        let done = sqlx::query!(
            r#"
            UPDATE bridge_user_host_model_prefs
            SET model_protocols = array_replace(model_protocols, $1, $2),
                updated_at = CURRENT_TIMESTAMP
            WHERE $1 = ANY(model_protocols)
            "#,
            from,
            to,
        )
        .execute(self.write_pool.as_ref())
        .await
        .map_err(RepositoryError::from)?;
        Ok(done.rows_affected())
    }

    pub async fn load_model_protocols(
        &self,
        user_id: &UserId,
    ) -> OauthResult<Vec<(HostKind, Vec<String>)>> {
        let rows = sqlx::query!(
            r#"
            SELECT host_id, model_protocols FROM bridge_user_host_model_prefs
            WHERE user_id = $1
            ORDER BY host_id
            "#,
            user_id as _,
        )
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                known_host(user_id, "bridge_user_host_model_prefs", &r.host_id)
                    .map(|host| (host, r.model_protocols))
            })
            .collect())
    }

    pub async fn set_model_protocols(
        &self,
        user_id: &UserId,
        host: HostKind,
        protocols: Option<&[String]>,
    ) -> OauthResult<()> {
        match protocols {
            Some(list) => {
                sqlx::query!(
                    r#"
                    INSERT INTO bridge_user_host_model_prefs
                        (user_id, host_id, model_protocols, updated_at)
                    VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
                    ON CONFLICT (user_id, host_id)
                    DO UPDATE SET model_protocols = EXCLUDED.model_protocols,
                                  updated_at = CURRENT_TIMESTAMP
                    "#,
                    user_id as _,
                    host.as_str(),
                    list,
                )
                .execute(self.write_pool.as_ref())
                .await?;
            },
            None => {
                sqlx::query!(
                    r#"
                    DELETE FROM bridge_user_host_model_prefs
                    WHERE user_id = $1 AND host_id = $2
                    "#,
                    user_id as _,
                    host.as_str(),
                )
                .execute(self.write_pool.as_ref())
                .await?;
            },
        }
        Ok(())
    }
}

#[async_trait]
impl BridgeHostFilterMigration for BridgeHostPrefsRepository {
    async fn rename_model_protocol_tag(
        &self,
        from: &str,
        to: &str,
    ) -> Result<u64, RepositoryError> {
        self.update_model_protocol_tag(from, to).await
    }
}
