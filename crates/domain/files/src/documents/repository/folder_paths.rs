//! The path history a folder rename or move writes for the live documents
//! beneath it, inside the tree-locked transaction that changes the folder.
//!
//! The documents' paths are read before the folder changes and logged as
//! `from`/`to` pairs after it, so the Drive feed reports each document under
//! the folder as renamed. A move also writes, per subject a grant on the
//! folder's former ancestors named, a `lost` row that lets the feed retract
//! the documents from devices that lost access with the old chain, and per
//! subject a grant on its new ancestors names that held nothing on a document
//! before, a `gained` row that lets the feed deliver it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_traits::RepositoryError;

use super::rows::RepoResult;

pub(super) struct SubtreePaths {
    ids: Vec<String>,
    paths: Vec<String>,
}

pub(super) async fn subtree_paths(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
) -> RepoResult<SubtreePaths> {
    let rows = sqlx::query!(
        r#"SELECT d.id::text AS "id!", document_drive_path(d.folder_id, d.name) AS "path!"
           FROM documents d
           JOIN document_folders f ON f.id = d.folder_id
           WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
             AND d.deleted_at IS NULL
           ORDER BY d.id"#,
        root.as_str(),
    )
    .fetch_all(tx)
    .await?;
    let (ids, paths) = rows.into_iter().map(|row| (row.id, row.path)).unzip();
    Ok(SubtreePaths { ids, paths })
}

pub(super) async fn log_subtree_paths(
    tx: &mut sqlx::PgConnection,
    before: &SubtreePaths,
) -> RepoResult<u64> {
    let logged = sqlx::query!(
        r#"INSERT INTO document_path_changes (document_id, from_path, to_path)
           SELECT d.id, b.from_path, document_drive_path(d.folder_id, d.name)
           FROM UNNEST($1::text[], $2::text[]) AS b(id, from_path)
           JOIN documents d ON d.id = b.id::uuid
           WHERE d.deleted_at IS NULL
             AND document_drive_path(d.folder_id, d.name) <> b.from_path"#,
        &before.ids,
        &before.paths,
    )
    .execute(tx)
    .await?;
    Ok(logged.rows_affected())
}

pub(super) async fn retract_ancestor_grants(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
) -> RepoResult<u64> {
    let written = sqlx::query!(
        r#"INSERT INTO document_path_changes
               (document_id, from_path, subject_kind, subject_value, subject_access)
           SELECT DISTINCT d.id, document_drive_path(d.folder_id, d.name),
                  g.subject_kind, g.subject_value, 'lost'
           FROM document_folders r
           JOIN document_grants g ON g.folder_id = ANY(r.ancestor_ids)
           JOIN document_folders f
             ON (f.id = r.id OR r.id = ANY(f.ancestor_ids)) AND f.deleted_at IS NULL
           JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
           WHERE r.id = $1::text::uuid"#,
        root.as_str(),
    )
    .execute(tx)
    .await?;
    Ok(written.rows_affected())
}

pub(super) async fn foreign_subtree_documents(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
    mover: &UserId,
) -> RepoResult<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT d.id::text AS "id!"
           FROM documents d
           JOIN document_folders f ON f.id = d.folder_id
           WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
             AND d.deleted_at IS NULL
             AND d.owner_user_id <> $2
           ORDER BY d.id
           FOR UPDATE OF d"#,
        root.as_str(),
        mover.as_str(),
    )
    .fetch_all(tx)
    .await
    .map_err(RepositoryError::from)
}

pub(super) async fn ancestors_of(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
) -> RepoResult<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT ancestor_ids::text[] AS "ancestors!" FROM document_folders
           WHERE id = $1::text::uuid"#,
        root.as_str(),
    )
    .fetch_optional(tx)
    .await
    .map(Option::unwrap_or_default)
    .map_err(RepositoryError::from)
}

pub(super) async fn deliver_new_ancestor_grants(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
    former_ancestors: &[String],
) -> RepoResult<u64> {
    let written = sqlx::query!(
        r#"INSERT INTO document_path_changes
               (document_id, to_path, subject_kind, subject_value, subject_access)
           SELECT DISTINCT d.id, document_drive_path(d.folder_id, d.name),
                  g.subject_kind, g.subject_value, 'gained'
           FROM document_folders r
           JOIN document_grants g
             ON g.folder_id = ANY(r.ancestor_ids)
            AND NOT (g.folder_id = ANY($2::text[]::uuid[]))
           JOIN document_folders f
             ON (f.id = r.id OR r.id = ANY(f.ancestor_ids)) AND f.deleted_at IS NULL
           JOIN documents d ON d.folder_id = f.id AND d.deleted_at IS NULL
           WHERE r.id = $1::text::uuid
             AND NOT (g.subject_kind = 'user' AND g.subject_value = d.owner_user_id)
             AND NOT EXISTS (
                 SELECT 1 FROM document_grants o
                 WHERE o.subject_kind = g.subject_kind
                   AND o.subject_value = g.subject_value
                   AND (o.document_id = d.id
                        OR o.folder_id = ANY($2::text[]::uuid[])
                        OR (o.folder_id = ANY(f.ancestor_ids || f.id)
                            AND NOT (o.folder_id = ANY(r.ancestor_ids)))))"#,
        root.as_str(),
        former_ancestors,
    )
    .execute(tx)
    .await?;
    Ok(written.rows_affected())
}
