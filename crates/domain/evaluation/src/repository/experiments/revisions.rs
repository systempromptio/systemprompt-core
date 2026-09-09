//! Immutable, owner-scoped evaluation resource snapshots.

use crate::Result;
use crate::experiments::resources::ResourceContent;
use sqlx::PgPool;
use systemprompt_identifiers::{EvalRevisionId, UserId};

use crate::experiments::{content_digest, invalid};

#[derive(Clone, Debug)]
pub struct RevisionRepository {
    pool: PgPool,
}

impl RevisionRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        owner: &UserId,
        key: &str,
        content: &ResourceContent,
    ) -> Result<EvalRevisionId> {
        content.validate()?;
        if let ResourceContent::Dataset(cases) = content {
            for case in cases {
                if !matches!(self.get(owner, case).await?, ResourceContent::Case(_)) {
                    return Err(invalid("Dataset members must be accessible case revisions"));
                }
            }
        }
        let kind = content.kind();
        if key.trim().is_empty() || key.len() > 200 {
            return Err(invalid("Invalid resource kind or key"));
        }
        if serde_json::to_vec(content)?.len() > 1_000_000 {
            return Err(invalid("Resource exceeds 1 MB"));
        }
        let digest = content_digest(content)?;
        let id = EvalRevisionId::generate();
        let existing: String = sqlx::query_scalar!("INSERT INTO eval_resource_revisions(id,owner_id,resource_kind,resource_key,digest,content) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(owner_id,resource_kind,resource_key,digest) DO UPDATE SET digest=EXCLUDED.digest RETURNING id", id.as_str(), owner.as_str(), kind, key, digest, sqlx::types::Json(content) as _)
            .fetch_one(&self.pool).await?;
        Ok(EvalRevisionId::new(existing))
    }

    pub async fn get(&self, owner: &UserId, id: &EvalRevisionId) -> Result<ResourceContent> {
        let content = sqlx::query_scalar!(
            r#"SELECT content AS "content!: sqlx::types::Json<ResourceContent>" FROM eval_resource_revisions WHERE id=$1 AND owner_id=$2"#,
            id.as_str(), owner.as_str()
        ).fetch_optional(&self.pool).await?
            .ok_or_else(|| crate::experiments::missing("Revision is unavailable in this scope"))?;
        Ok(content.0)
    }
}
