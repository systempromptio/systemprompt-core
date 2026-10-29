//! The root `folder` listing scope behind
//! [`DocumentRepository::list_scoped`]: root-level documents the caller owns
//! or holds a document grant on. A folder grant never reaches the root, so the
//! candidate set is the caller's own documents and the documents their subject
//! pairs are granted, and only those are ranked and paged.
//! [`ROOT_LISTING_SQL`] is the statement text, exposed so its plan can be
//! checked against the production statement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};

use super::documents::DocumentRepository;
use super::listing::{ListArgs, ListedRow};
use super::rows::RepoResult;

pub const ROOT_LISTING_SQL: &str = r#"WITH subject AS (
                   SELECT s.kind, s.value FROM UNNEST($8::text[], $9::text[]) AS s(kind, value)
               ),
               candidate AS (
                   SELECT d.id, 5 AS rank FROM documents d
                   WHERE d.owner_user_id = $1 AND d.folder_id IS NULL AND d.deleted_at IS NULL
                   UNION ALL
                   SELECT g.document_id,
                          array_position(ARRAY['view','comment','edit','share','own'], g.access_level)
                   FROM document_grants g
                   JOIN subject s ON s.kind = g.subject_kind AND s.value = g.subject_value
                   WHERE g.document_id IS NOT NULL
                     AND (g.valid_until IS NULL OR g.valid_until > now())
               ),
               ranked AS (
                   SELECT d.id, d.name, d.updated_at, MAX(c.rank) AS rank,
                          CASE WHEN $3 = 'recent' THEN GREATEST(
                   (SELECT MAX(r.created_at) FROM document_revisions r
                    WHERE r.document_id = d.id AND r.author_user_id = $1),
                   (SELECT MAX(cm.created_at) FROM document_comments cm
                    WHERE cm.document_id = d.id AND cm.author_user_id = $1),
                   (SELECT MAX(pr.updated_at) FROM document_presence pr
                    WHERE pr.document_id = d.id AND pr.user_id = $1)) END AS sort_touched
                   FROM candidate c
                   JOIN documents d ON d.id = c.id
                   WHERE d.folder_id IS NULL AND d.deleted_at IS NULL
                     AND ($2::text IS NULL OR d.name ILIKE $2)
                   GROUP BY d.id
               ),
               page AS (
                   SELECT k.id, k.rank, k.sort_touched FROM ranked k
                   WHERE k.rank IS NOT NULL
                     AND ($6::uuid IS NULL OR CASE $3
                           WHEN 'name' THEN (k.name, k.id) > ($5::text, $6::uuid)
                           WHEN 'recent' THEN (COALESCE(k.sort_touched, 'epoch'::timestamptz), k.id)
                                              < ($4::timestamptz, $6::uuid)
                           ELSE (k.updated_at, k.id) < ($4::timestamptz, $6::uuid) END)
                   ORDER BY CASE WHEN $3 = 'name' THEN k.name END ASC,
                            CASE WHEN $3 = 'name' THEN k.id END ASC,
                            CASE WHEN $3 = 'recent'
                                 THEN COALESCE(k.sort_touched, 'epoch'::timestamptz) END DESC,
                            CASE WHEN $3 = 'updated' THEN k.updated_at END DESC,
                            k.id DESC
                   LIMIT $7
               )
               SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId",
                      b.size_bytes AS "size_bytes?",
                      CASE WHEN d.owner_user_id = $1 THEN 'own'
                           ELSE (ARRAY['view','comment','edit','share','share'])[p.rank]
                      END AS my_access,
                      GREATEST(
                   (SELECT MAX(r.created_at) FROM document_revisions r
                    WHERE r.document_id = d.id AND r.author_user_id = $1),
                   (SELECT MAX(c.created_at) FROM document_comments c
                    WHERE c.document_id = d.id AND c.author_user_id = $1),
                   (SELECT MAX(pr.updated_at) FROM document_presence pr
                    WHERE pr.document_id = d.id AND pr.user_id = $1)) AS touched_at
               FROM page p
               JOIN documents d ON d.id = p.id
               LEFT JOIN document_revisions r ON r.id = d.head_revision_id
               LEFT JOIN document_blobs b ON b.content_sha256 = r.content_sha256
               ORDER BY CASE WHEN $3 = 'name' THEN d.name END ASC,
                        CASE WHEN $3 = 'name' THEN d.id END ASC,
                        CASE WHEN $3 = 'recent'
                             THEN COALESCE(p.sort_touched, 'epoch'::timestamptz) END DESC,
                        CASE WHEN $3 = 'updated' THEN d.updated_at END DESC,
                        d.id DESC"#;

impl DocumentRepository {
    #[expect(
        clippy::too_many_lines,
        reason = "one query! literal; each scope is a single SQL statement"
    )]
    pub(super) async fn list_at_root(&self, args: &ListArgs<'_>) -> RepoResult<Vec<ListedRow>> {
        Ok(sqlx::query_as!(
            ListedRow,
            r#"WITH subject AS (
                   SELECT s.kind, s.value FROM UNNEST($8::text[], $9::text[]) AS s(kind, value)
               ),
               candidate AS (
                   SELECT d.id, 5 AS rank FROM documents d
                   WHERE d.owner_user_id = $1 AND d.folder_id IS NULL AND d.deleted_at IS NULL
                   UNION ALL
                   SELECT g.document_id,
                          array_position(ARRAY['view','comment','edit','share','own'], g.access_level)
                   FROM document_grants g
                   JOIN subject s ON s.kind = g.subject_kind AND s.value = g.subject_value
                   WHERE g.document_id IS NOT NULL
                     AND (g.valid_until IS NULL OR g.valid_until > now())
               ),
               ranked AS (
                   SELECT d.id, d.name, d.updated_at, MAX(c.rank) AS rank,
                          CASE WHEN $3 = 'recent' THEN GREATEST(
                   (SELECT MAX(r.created_at) FROM document_revisions r
                    WHERE r.document_id = d.id AND r.author_user_id = $1),
                   (SELECT MAX(cm.created_at) FROM document_comments cm
                    WHERE cm.document_id = d.id AND cm.author_user_id = $1),
                   (SELECT MAX(pr.updated_at) FROM document_presence pr
                    WHERE pr.document_id = d.id AND pr.user_id = $1)) END AS sort_touched
                   FROM candidate c
                   JOIN documents d ON d.id = c.id
                   WHERE d.folder_id IS NULL AND d.deleted_at IS NULL
                     AND ($2::text IS NULL OR d.name ILIKE $2)
                   GROUP BY d.id
               ),
               page AS (
                   SELECT k.id, k.rank, k.sort_touched FROM ranked k
                   WHERE k.rank IS NOT NULL
                     AND ($6::uuid IS NULL OR CASE $3
                           WHEN 'name' THEN (k.name, k.id) > ($5::text, $6::uuid)
                           WHEN 'recent' THEN (COALESCE(k.sort_touched, 'epoch'::timestamptz), k.id)
                                              < ($4::timestamptz, $6::uuid)
                           ELSE (k.updated_at, k.id) < ($4::timestamptz, $6::uuid) END)
                   ORDER BY CASE WHEN $3 = 'name' THEN k.name END ASC,
                            CASE WHEN $3 = 'name' THEN k.id END ASC,
                            CASE WHEN $3 = 'recent'
                                 THEN COALESCE(k.sort_touched, 'epoch'::timestamptz) END DESC,
                            CASE WHEN $3 = 'updated' THEN k.updated_at END DESC,
                            k.id DESC
                   LIMIT $7
               )
               SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.deleted_at AS trashed_at,
                      d.trashed_by AS "trashed_by: UserId",
                      b.size_bytes AS "size_bytes?",
                      CASE WHEN d.owner_user_id = $1 THEN 'own'
                           ELSE (ARRAY['view','comment','edit','share','share'])[p.rank]
                      END AS my_access,
                      GREATEST(
                   (SELECT MAX(r.created_at) FROM document_revisions r
                    WHERE r.document_id = d.id AND r.author_user_id = $1),
                   (SELECT MAX(c.created_at) FROM document_comments c
                    WHERE c.document_id = d.id AND c.author_user_id = $1),
                   (SELECT MAX(pr.updated_at) FROM document_presence pr
                    WHERE pr.document_id = d.id AND pr.user_id = $1)) AS touched_at
               FROM page p
               JOIN documents d ON d.id = p.id
               LEFT JOIN document_revisions r ON r.id = d.head_revision_id
               LEFT JOIN document_blobs b ON b.content_sha256 = r.content_sha256
               ORDER BY CASE WHEN $3 = 'name' THEN d.name END ASC,
                        CASE WHEN $3 = 'name' THEN d.id END ASC,
                        CASE WHEN $3 = 'recent'
                             THEN COALESCE(p.sort_touched, 'epoch'::timestamptz) END DESC,
                        CASE WHEN $3 = 'updated' THEN d.updated_at END DESC,
                        d.id DESC"#,
            args.caller.as_str(),
            args.pattern.as_deref(),
            args.sort,
            args.key.at,
            args.key.name.as_deref(),
            args.key.id,
            args.limit,
            &args.subjects.kinds,
            &args.subjects.values,
        )
        .fetch_all(self.pools.authz())
        .await?)
    }
}
