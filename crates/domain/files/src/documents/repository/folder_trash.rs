//! The statements a subtree trash runs inside its tree-locked transaction:
//! the stray check, trashing the partitioned documents, re-homing the
//! collaborators' documents to their owners' roots, and the live-document
//! re-check.
//!
//! A re-homed document leaves its folder chain's grants behind, so the
//! re-home also writes a path history row per subject those grants named,
//! for the Drive feed to retract it from devices that lost access.
//!
//! Each write re-checks the owner the caller partitioned by, so a document
//! whose ownership changed after the partition was read is neither trashed
//! for its new owner nor re-homed away from the deleter; the caller compares
//! the counts with its plan and rolls back on any difference.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, UserId};
use systemprompt_traits::RepositoryError;

use super::rows::RepoResult;

pub(super) async fn subtree_has_stray(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
    partitioned: &[String],
    folders: &[String],
) -> RepoResult<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM documents d
               JOIN document_folders f ON f.id = d.folder_id
               WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND d.deleted_at IS NULL
                 AND NOT (d.id = ANY($2::text[]::uuid[]))
           ) OR EXISTS (
               SELECT 1 FROM document_folders f
               WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND f.deleted_at IS NULL
                 AND NOT (f.id = ANY($3::text[]::uuid[]))
           ) AS "stray!""#,
        root.as_str(),
        &partitioned,
        folders,
    )
    .fetch_one(tx)
    .await
    .map_err(RepositoryError::from)
}

pub(super) async fn trash_subtree_documents(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
    by: &UserId,
    authorized: &[String],
) -> RepoResult<i64> {
    sqlx::query_scalar!(
        r#"WITH trashed AS (
               UPDATE documents d
               SET deleted_at = now(), trashed_by = $2, updated_at = now()
               FROM document_folders f
               WHERE f.id = d.folder_id
                 AND (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND d.deleted_at IS NULL
                 AND d.owner_user_id = $2
                 AND d.id = ANY($3::text[]::uuid[])
               RETURNING d.id, d.name, f.ancestor_ids || f.id AS chain),
           logged AS (
               INSERT INTO document_path_changes (document_id, from_path)
               SELECT t.id,
                      (SELECT string_agg(a.name, '/' ORDER BY u.ord)
                       FROM unnest(t.chain) WITH ORDINALITY AS u(folder_id, ord)
                       JOIN document_folders a ON a.id = u.folder_id) || '/' || t.name
               FROM trashed t RETURNING 1)
           SELECT COUNT(*) AS "trashed!" FROM trashed"#,
        root.as_str(),
        by.as_str(),
        authorized,
    )
    .fetch_one(tx)
    .await
    .map_err(RepositoryError::from)
}

pub(super) async fn rehome_subtree_documents(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
    by: &UserId,
    rehome: &[String],
) -> RepoResult<i64> {
    sqlx::query_scalar!(
        r#"WITH homed AS (
               UPDATE documents d
               SET folder_id = NULL, updated_at = now()
               FROM document_folders f
               WHERE f.id = d.folder_id
                 AND (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND d.deleted_at IS NULL
                 AND d.owner_user_id <> $3
                 AND d.id = ANY($2::text[]::uuid[])
               RETURNING d.id, d.name, f.ancestor_ids || f.id AS chain),
           logged AS (
               INSERT INTO document_path_changes (document_id, from_path, to_path)
               SELECT h.id,
                      (SELECT string_agg(a.name, '/' ORDER BY u.ord)
                       FROM unnest(h.chain) WITH ORDINALITY AS u(folder_id, ord)
                       JOIN document_folders a ON a.id = u.folder_id) || '/' || h.name,
                      h.name
               FROM homed h RETURNING 1),
           lost AS (
               INSERT INTO document_path_changes
                   (document_id, from_path, subject_kind, subject_value, subject_access)
               SELECT DISTINCT h.id,
                      (SELECT string_agg(a.name, '/' ORDER BY u.ord)
                       FROM unnest(h.chain) WITH ORDINALITY AS u(folder_id, ord)
                       JOIN document_folders a ON a.id = u.folder_id) || '/' || h.name,
                      g.subject_kind, g.subject_value, 'lost'
               FROM homed h
               JOIN document_grants g ON g.folder_id = ANY(h.chain)
               RETURNING 1)
           SELECT COUNT(*) AS "rehomed!" FROM homed"#,
        root.as_str(),
        rehome,
        by.as_str(),
    )
    .fetch_one(tx)
    .await
    .map_err(RepositoryError::from)
}

pub(super) async fn subtree_has_live_documents(
    tx: &mut sqlx::PgConnection,
    root: &DocumentFolderId,
) -> RepoResult<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (
               SELECT 1 FROM documents d
               JOIN document_folders f ON f.id = d.folder_id
               WHERE (f.id = $1::text::uuid OR $1::text::uuid = ANY(f.ancestor_ids))
                 AND d.deleted_at IS NULL
           ) AS "remaining!""#,
        root.as_str(),
    )
    .fetch_one(tx)
    .await
    .map_err(RepositoryError::from)
}
