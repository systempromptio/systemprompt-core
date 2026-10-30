//! [`PipelineVersionRepository`]: every pipeline version the brain has run
//! under, with the inputs it was derived from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::PipelineVersion;
use systemprompt_models::brain::PipelineVersionInputs;
use systemprompt_traits::RepositoryError;

use super::Pools;

#[derive(Debug, Clone)]
pub struct PipelineVersionRepository {
    pools: Pools,
}

impl PipelineVersionRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn record(
        conn: &mut PgConnection,
        inputs: &PipelineVersionInputs,
    ) -> Result<PipelineVersion, RepositoryError> {
        let version = inputs.derive();
        let code_version = i32::try_from(inputs.code_version).map_err(|_| {
            RepositoryError::invalid_argument("code_version", "exceeds the column range")
        })?;
        sqlx::query!(
            "INSERT INTO brain_pipeline_versions
                 (version, declared_hash, code_version, prompt_versions)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (version) DO NOTHING",
            version.as_str(),
            inputs.declared_hash,
            code_version,
            serde_json::to_value(&inputs.prompt_versions)?,
        )
        .execute(conn)
        .await?;
        Ok(version)
    }

    pub async fn current(
        executor: impl PgExecutor<'_>,
    ) -> Result<Option<PipelineVersion>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT version AS "version: PipelineVersion" FROM brain_pipeline_versions
               ORDER BY recorded_at DESC, version DESC LIMIT 1"#
        )
        .fetch_optional(executor)
        .await?)
    }

    pub async fn current_version(&self) -> Result<Option<PipelineVersion>, RepositoryError> {
        Self::current(&*self.pools.read).await
    }
}
