//! The `context` listing scope behind [`DocumentRepository::list_scoped`]:
//! the documents one conversation touched, meaning a revision or a comment
//! that carries its context id. Candidates come from the
//! `(context_id, created_at)` indexes on revisions and comments; each is then
//! ranked for the caller exactly as the other scopes rank (owner, or the best
//! live grant on the document or its folder chain), and a document the caller
//! can no longer see drops out. A row's touch time is the conversation's last
//! revision or comment on it. The parameter layout is the sibling
//! `listing_scopes` module's, with `$10` the context id.
//!
//! Whether the caller owns the conversation is decided before this runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    ContextId, DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};

use super::documents::DocumentRepository;
use super::listing::{ListArgs, ListedRow};
use super::rows::RepoResult;

impl DocumentRepository {
    #[expect(
        clippy::too_many_lines,
        reason = "one query! literal; each scope is a single SQL statement"
    )]
    pub(super) async fn list_in_context(
        &self,
        context: &ContextId,
        args: &ListArgs<'_>,
    ) -> RepoResult<Vec<ListedRow>> {
        Ok(sqlx::query_as!(
            ListedRow,
            r#"WITH touched AS (
                   SELECT t.document_id, MAX(t.at) AS touched_at FROM (
                       SELECT r.document_id, r.created_at AS at FROM document_revisions r
                       WHERE r.context_id = $10
                       UNION ALL
                       SELECT c.document_id, c.created_at FROM document_comments c
                       WHERE c.context_id = $10
                   ) t
                   GROUP BY t.document_id
               ),
               ranked AS (
                   SELECT d.id, d.name, d.updated_at, t.touched_at AS sort_touched,
                          CASE WHEN d.owner_user_id = $1 THEN 5 ELSE (
                              SELECT MAX(array_position(ARRAY['view','comment','edit','share','own'], g.access_level))
                              FROM document_grants g
                              LEFT JOIN document_folders f ON f.id = d.folder_id
                              WHERE (g.document_id = d.id
                                     OR g.folder_id = ANY(f.ancestor_ids || f.id))
                                AND (g.subject_kind, g.subject_value) IN (
                                    SELECT * FROM UNNEST($8::text[], $9::text[]))
                                AND (g.valid_until IS NULL OR g.valid_until > now()))
                          END AS rank
                   FROM touched t
                   JOIN documents d ON d.id = t.document_id
                   WHERE d.deleted_at IS NULL AND ($2::text IS NULL OR d.name ILIKE $2)
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
                      p.sort_touched AS touched_at
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
            args.caller as _,
            args.pattern.as_deref(),
            args.sort,
            args.key.at,
            args.key.name.as_deref(),
            args.key.id,
            args.limit,
            &args.subjects.kinds,
            &args.subjects.values,
            context as _,
        )
        .fetch_all(self.pools.authz())
        .await?)
    }
}
