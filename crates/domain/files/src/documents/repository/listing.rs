//! Scoped document listings: a folder, documents shared with the caller,
//! the caller's own documents, or the documents the caller touched last.
//!
//! One statement decides visibility, the scope, the escaped name filter, the
//! ordering and the keyset page, and computes each row's head blob size and
//! the caller's grant level. A grant reaches a document directly or through
//! any folder in its ancestor chain, and matches any of the caller's subject
//! pairs, exactly as access checks resolve it. A touch is a revision the
//! caller authored, a comment the caller wrote, or the caller's presence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::{
    DocumentFolderId, DocumentId, DocumentRevisionId, DocumentTemplateId, UserId,
};
use uuid::Uuid;

use super::documents::{DocumentRepository, DocumentRow};
use super::rows::{RepoResult, parse};
use crate::documents::models::{
    Document, ListCursor, ListSort, ListedDocument, ListingCursor, ScopedListing, SubjectPairs,
};

struct ListedRow {
    id: DocumentId,
    folder_id: Option<DocumentFolderId>,
    name: String,
    format: String,
    mime_type: String,
    head_revision_id: Option<DocumentRevisionId>,
    template_id: Option<DocumentTemplateId>,
    owner_user_id: UserId,
    created_by: UserId,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    trashed_at: Option<DateTime<Utc>>,
    trashed_by: Option<UserId>,
    size_bytes: Option<i64>,
    my_access: Option<String>,
    touched_at: Option<DateTime<Utc>>,
}

impl ListedRow {
    fn into_listed(self) -> RepoResult<ListedDocument> {
        let my_access = parse(
            "documents.my_access",
            self.my_access.as_deref().unwrap_or("view"),
        )?;
        let document = Document::try_from(DocumentRow {
            id: self.id,
            folder_id: self.folder_id,
            name: self.name,
            format: self.format,
            mime_type: self.mime_type,
            head_revision_id: self.head_revision_id,
            template_id: self.template_id,
            owner_user_id: self.owner_user_id,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
            trashed_at: self.trashed_at,
            trashed_by: self.trashed_by,
        })?;
        Ok(ListedDocument {
            document,
            size_bytes: self.size_bytes,
            my_access,
            touched_at: self.touched_at,
        })
    }
}

fn like_pattern(query: Option<&str>) -> Option<String> {
    query.map(str::trim).filter(|q| !q.is_empty()).map(|q| {
        format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    })
}

struct Keyset {
    at: Option<DateTime<Utc>>,
    name: Option<String>,
    id: Option<Uuid>,
}

fn keyset(cursor: Option<&ListingCursor>) -> Keyset {
    match cursor {
        Some(ListingCursor::Time(c)) => Keyset {
            at: Some(c.updated_at),
            name: None,
            id: Some(c.id),
        },
        Some(ListingCursor::Name { name, id }) => Keyset {
            at: None,
            name: Some(name.clone()),
            id: Some(*id),
        },
        None => Keyset {
            at: None,
            name: None,
            id: None,
        },
    }
}

fn next_cursor(sort: ListSort, last: &ListedDocument) -> RepoResult<ListingCursor> {
    let id = Uuid::parse_str(last.document.id.as_str())
        .map_err(|source| systemprompt_traits::RepositoryError::decode("documents.id", source))?;
    Ok(match sort {
        ListSort::Name => ListingCursor::Name {
            name: last.document.name.clone(),
            id,
        },
        ListSort::Updated => ListingCursor::Time(ListCursor::new(last.document.updated_at, id)),
        ListSort::Recent => ListingCursor::Time(ListCursor::new(
            last.touched_at.unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
            id,
        )),
    })
}

impl DocumentRepository {
    #[expect(
        clippy::too_many_lines,
        reason = "one query! literal; the scoped listing is a single SQL statement"
    )]
    pub async fn list_scoped(
        &self,
        listing: &ScopedListing,
        subjects: &SubjectPairs,
        caller: &UserId,
    ) -> RepoResult<(Vec<ListedDocument>, Option<ListingCursor>)> {
        let limit = listing.limit.max(1);
        let key = keyset(listing.cursor.as_ref());
        let rows = sqlx::query_as!(
            ListedRow,
            r#"WITH access AS (
                   SELECT d.id AS document_id,
                          CASE WHEN d.owner_user_id = $1 THEN 5 ELSE (
                              SELECT MAX(array_position(
                                  ARRAY['view','comment','edit','share','own'], g.access_level))
                              FROM document_grants g
                              LEFT JOIN document_folders f ON f.id = d.folder_id
                              WHERE (g.document_id = d.id
                                     OR g.folder_id = ANY(f.ancestor_ids || f.id))
                                AND (g.subject_kind, g.subject_value) IN (
                                    SELECT * FROM UNNEST($2::text[], $3::text[]))
                                AND (g.valid_until IS NULL OR g.valid_until > now()))
                          END AS rank,
                          GREATEST(
                              (SELECT MAX(r.created_at) FROM document_revisions r
                               WHERE r.document_id = d.id AND r.author_user_id = $1),
                              (SELECT MAX(c.created_at) FROM document_comments c
                               WHERE c.document_id = d.id AND c.author_user_id = $1),
                              (SELECT MAX(p.updated_at) FROM document_presence p
                               WHERE p.document_id = d.id AND p.user_id = $1)) AS touched_at
                   FROM documents d
                   WHERE d.trashed_at IS NULL
                     AND ($4 <> 'folder' OR d.folder_id IS NOT DISTINCT FROM $5::text::uuid)
                     AND ($4 <> 'mine' OR d.owner_user_id = $1)
                     AND ($4 <> 'shared' OR d.owner_user_id <> $1)
                     AND ($6::text IS NULL OR d.name ILIKE $6)
               )
               SELECT d.id::text AS "id!: DocumentId",
                      d.folder_id::text AS "folder_id: DocumentFolderId",
                      d.name, d.format, d.mime_type,
                      d.head_revision_id::text AS "head_revision_id: DocumentRevisionId",
                      d.template_id AS "template_id: DocumentTemplateId",
                      d.owner_user_id AS "owner_user_id: UserId",
                      d.created_by AS "created_by: UserId",
                      d.created_at, d.updated_at, d.trashed_at,
                      d.trashed_by AS "trashed_by: UserId",
                      b.size_bytes AS "size_bytes?",
                      (ARRAY['view','comment','edit','share','own'])[a.rank] AS my_access,
                      a.touched_at
               FROM access a
               JOIN documents d ON d.id = a.document_id
               LEFT JOIN document_revisions r ON r.id = d.head_revision_id
               LEFT JOIN document_blobs b ON b.content_sha256 = r.content_sha256
               WHERE a.rank IS NOT NULL
                 AND ($4 <> 'recent' OR a.touched_at IS NOT NULL)
                 AND ($10::uuid IS NULL OR CASE $7
                       WHEN 'name' THEN (d.name, d.id) > ($9::text, $10::uuid)
                       WHEN 'recent' THEN (COALESCE(a.touched_at, 'epoch'::timestamptz), d.id)
                                          < ($8::timestamptz, $10::uuid)
                       ELSE (d.updated_at, d.id) < ($8::timestamptz, $10::uuid) END)
               ORDER BY CASE WHEN $7 = 'name' THEN d.name END ASC,
                        CASE WHEN $7 = 'name' THEN d.id END ASC,
                        CASE WHEN $7 = 'recent' THEN COALESCE(a.touched_at, 'epoch'::timestamptz) END DESC,
                        CASE WHEN $7 = 'updated' THEN d.updated_at END DESC,
                        d.id DESC
               LIMIT $11"#,
            caller.as_str(),
            &subjects.kinds,
            &subjects.values,
            listing.scope.as_str(),
            listing.folder_id.as_ref().map(DocumentFolderId::as_str),
            like_pattern(listing.query.as_deref()),
            listing.sort.as_str(),
            key.at,
            key.name,
            key.id,
            limit + 1,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        let mut documents = rows
            .into_iter()
            .map(ListedRow::into_listed)
            .collect::<RepoResult<Vec<_>>>()?;
        let next = if documents.len() as i64 > limit {
            documents.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
            documents
                .last()
                .map(|last| next_cursor(listing.sort, last))
                .transpose()?
        } else {
            None
        };
        Ok((documents, next))
    }
}
