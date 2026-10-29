//! Scoped document listings: a folder, documents shared with the caller,
//! the caller's own documents, or the documents the caller touched last.
//!
//! Each scope has its own statement that first builds a bounded candidate
//! set from an index (the folder's documents, the caller's own, the
//! documents the caller's grants reach directly or through a folder chain, or
//! the documents the caller authored, commented on or is present in), then
//! ranks and pages it, and only then reads the head blob size and the touch
//! time for the rows on the page. A grant matches any of the caller's subject
//! pairs, exactly as access checks resolve it, and a stored `own` grant reports
//! as `share`, as resolution caps it; only the owner reports `own`.
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
    Document, ListCursor, ListScope, ListSort, ListedDocument, ListingCursor, ScopedListing,
    SubjectPairs,
};

pub(super) struct ListedRow {
    pub(super) id: DocumentId,
    pub(super) folder_id: Option<DocumentFolderId>,
    pub(super) name: String,
    pub(super) format: String,
    pub(super) mime_type: String,
    pub(super) head_revision_id: Option<DocumentRevisionId>,
    pub(super) template_id: Option<DocumentTemplateId>,
    pub(super) owner_user_id: UserId,
    pub(super) created_by: UserId,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) trashed_at: Option<DateTime<Utc>>,
    pub(super) trashed_by: Option<UserId>,
    pub(super) size_bytes: Option<i64>,
    pub(super) my_access: Option<String>,
    pub(super) touched_at: Option<DateTime<Utc>>,
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

pub(crate) fn like_pattern(query: Option<&str>) -> Option<String> {
    query.map(str::trim).filter(|q| !q.is_empty()).map(|q| {
        format!(
            "%{}%",
            q.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    })
}

pub(super) struct Keyset {
    pub(super) at: Option<DateTime<Utc>>,
    pub(super) name: Option<String>,
    pub(super) id: Option<Uuid>,
}

pub(super) struct ListArgs<'a> {
    pub(super) caller: &'a UserId,
    pub(super) subjects: &'a SubjectPairs,
    pub(super) pattern: Option<String>,
    pub(super) sort: &'static str,
    pub(super) key: Keyset,
    pub(super) limit: i64,
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
    pub async fn list_scoped(
        &self,
        listing: &ScopedListing,
        subjects: &SubjectPairs,
        caller: &UserId,
    ) -> RepoResult<(Vec<ListedDocument>, Option<ListingCursor>)> {
        let limit = listing.limit.max(1);
        let args = ListArgs {
            caller,
            subjects,
            pattern: like_pattern(listing.query.as_deref()),
            sort: listing.sort.as_str(),
            key: keyset(listing.cursor.as_ref()),
            limit: limit + 1,
        };
        let rows = match (listing.scope, listing.folder_id.as_ref()) {
            (ListScope::Folder, Some(folder)) => self.list_in_folder(folder, &args).await?,
            (ListScope::Folder, None) => self.list_at_root(&args).await?,
            (ListScope::Mine, _) => self.list_mine(&args).await?,
            (ListScope::Shared, _) => self.list_shared(&args).await?,
            (ListScope::Recent, _) => self.list_recent(&args).await?,
        };
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
