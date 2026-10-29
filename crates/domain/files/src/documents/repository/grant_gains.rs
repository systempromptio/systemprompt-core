//! The `gained` path history a grant write records before it lands, on the
//! grant's own connection: one row per live document the grant covers whose
//! subject held nothing on it before (no grant on the document or its folder
//! chain, and not its owner), so the Drive feed can deliver the document to
//! that subject's devices. An upgrade of a grant the subject already held
//! records nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;

use super::rows::RepoResult;
use crate::documents::models::{GrantTarget, NewGrant};

pub(super) async fn record_gained(conn: &mut PgConnection, grant: &NewGrant) -> RepoResult<u64> {
    let (document_id, folder_id) = match &grant.target {
        GrantTarget::Document(id) => (Some(id.as_str()), None),
        GrantTarget::Folder(id) => (None, Some(id.as_str())),
    };
    let written = sqlx::query!(
        r#"WITH covered AS (
               SELECT d.id, d.folder_id, d.name, d.owner_user_id,
                      COALESCE(f.ancestor_ids || f.id, '{}'::uuid[]) AS chain
               FROM documents d
               LEFT JOIN document_folders f ON f.id = d.folder_id
               WHERE d.id = $1::text::uuid AND d.deleted_at IS NULL
               UNION ALL
               SELECT d.id, d.folder_id, d.name, d.owner_user_id, f.ancestor_ids || f.id
               FROM document_folders f
               JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
               WHERE $2::text IS NOT NULL
                 AND (f.id = $2::text::uuid OR $2::text::uuid = ANY(f.ancestor_ids))
                 AND f.deleted_at IS NULL)
           INSERT INTO document_path_changes
               (document_id, to_path, subject_kind, subject_value, subject_access)
           SELECT c.id, document_drive_path(c.folder_id, c.name), $3, $4, 'gained'
           FROM covered c
           WHERE NOT ($3 = 'user' AND $4 = c.owner_user_id)
             AND NOT EXISTS (
                 SELECT 1 FROM document_grants o
                 WHERE o.subject_kind = $3 AND o.subject_value = $4
                   AND (o.document_id = c.id OR o.folder_id = ANY(c.chain)))"#,
        document_id,
        folder_id,
        grant.subject_kind,
        grant.subject_value,
    )
    .execute(conn)
    .await?;
    Ok(written.rows_affected())
}
