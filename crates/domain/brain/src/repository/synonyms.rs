//! [`SynonymsRepository`]: the `synonyms` config document as a typed
//! [`SynonymsDoc`]; no row means no groups.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Deserialize;
use sqlx::PgExecutor;
use systemprompt_models::brain::SynonymsDoc;
use systemprompt_traits::RepositoryError;

pub const SYNONYMS_DOC_ID: &str = "synonyms";

#[derive(Debug, Clone, Copy, Default)]
pub struct SynonymsRepository;

impl SynonymsRepository {
    pub async fn load(executor: impl PgExecutor<'_>) -> Result<SynonymsDoc, RepositoryError> {
        let body = sqlx::query_scalar!(
            "SELECT body FROM brain_config_docs WHERE id = $1",
            SYNONYMS_DOC_ID
        )
        .fetch_optional(executor)
        .await?;
        body.map_or_else(
            || Ok(SynonymsDoc::default()),
            |body| {
                SynonymsDoc::deserialize(&body)
                    .map_err(|e| RepositoryError::decode("brain_config_docs.synonyms", e))
            },
        )
    }
}
