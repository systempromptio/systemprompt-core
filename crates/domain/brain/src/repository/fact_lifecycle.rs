//! Source-driven staleness: a landed revision stales the facts built on
//! earlier revisions of its document, and a resolved comment stales the
//! facts built on it. Both lock the affected rows and stale each one
//! through [`FactRepository::apply`], so every stale mark is a revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{BrainFactId, DocumentCommentId, DocumentId, DocumentRevisionId};
use systemprompt_traits::RepositoryError;

use super::FactRepository;
use crate::models::FactChange;

impl FactRepository {
    pub async fn mark_stale_for_document(
        conn: &mut PgConnection,
        document_id: &DocumentId,
        except_revision: &DocumentRevisionId,
    ) -> Result<Vec<BrainFactId>, RepositoryError> {
        let ids = sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE source_document_id = $1
                 AND source_kind = 'document'
                 AND source_revision_id IS DISTINCT FROM $2
                 AND status NOT IN ('stale', 'superseded', 'retired')
               ORDER BY id FOR UPDATE"#,
            document_id.as_str(),
            except_revision.as_str(),
        )
        .fetch_all(&mut *conn)
        .await?;
        Self::stale_all(conn, ids).await
    }

    pub async fn mark_stale_for_comment(
        conn: &mut PgConnection,
        comment_id: &DocumentCommentId,
    ) -> Result<Vec<BrainFactId>, RepositoryError> {
        let ids = sqlx::query_scalar!(
            r#"SELECT id AS "id: BrainFactId" FROM brain_facts
               WHERE source_comment_id = $1
                 AND status NOT IN ('stale', 'superseded', 'retired')
               ORDER BY id FOR UPDATE"#,
            comment_id.as_str(),
        )
        .fetch_all(&mut *conn)
        .await?;
        Self::stale_all(conn, ids).await
    }

    async fn stale_all(
        conn: &mut PgConnection,
        ids: Vec<BrainFactId>,
    ) -> Result<Vec<BrainFactId>, RepositoryError> {
        for id in &ids {
            Self::apply(&mut *conn, id, FactChange::Staled).await?;
        }
        Ok(ids)
    }
}
