//! Every registered Git source that names a credential label, across owners.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::types::Json;
use systemprompt_identifiers::ManagedSourceId;

use super::ManagedRepository;
use crate::managed::{Result, SourceSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCredentialSource {
    pub id: ManagedSourceId,
    pub name: String,
    pub repository: String,
    pub credential_reference: String,
}

impl ManagedRepository {
    pub async fn git_sources_with_credentials(&self) -> Result<Vec<GitCredentialSource>> {
        let rows = sqlx::query!(
            r#"SELECT id, name, specification AS "specification!: Json<SourceSpec>"
               FROM managed_sources WHERE kind = 'git' ORDER BY name, id"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| match row.specification.0 {
                SourceSpec::Git {
                    repository,
                    credential_reference: Some(label),
                    ..
                } => Some(GitCredentialSource {
                    id: ManagedSourceId::new(row.id),
                    name: row.name,
                    repository,
                    credential_reference: label,
                }),
                _ => None,
            })
            .collect())
    }
}
