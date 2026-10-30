//! Stable node ids of revisions: read from the store, never minted on a read.
//!
//! A commit records the new revision's ids in the transaction that writes
//! the revision, so every revision written since ids exist has them. A
//! revision written before has none until the `document_node_ids_backfill`
//! job mints them with [`DocumentService::backfill_node_ids`]: carried from
//! its parent when the parent has ids, in a transaction that locks the
//! revision row, so two minters never write it twice. Until then a
//! projection of that revision refuses `not_found`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.


use systemprompt_models::documents::{DocumentFormat, Projection};
use systemprompt_traits::RepositoryError;

use super::DocumentService;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NodeIdEntry};
use crate::documents::nodes::{
    CarryHints, NodeIdMinter, carry_forward, mint_all, with_ids, writer_tag,
};
use crate::documents::repository::DocumentNodeIdRepository;

#[derive(Debug, Clone)]
pub(super) struct Projected {
    pub paths: Projection,
    pub ids: Projection,
}

impl DocumentService {
    pub(super) async fn project_paths(
        &self,
        format: DocumentFormat,
        bytes: Vec<u8>,
    ) -> Result<Projection, DocumentsError> {
        let codec = self.codec(format)?;
        blocking(move || Ok(codec.project(&bytes)?)).await
    }

    /// The projection of `revision` with its stored node ids, for a live
    /// session seeded from it.
    pub(crate) async fn projection_with_ids(
        &self,
        document: &Document,
        revision: &systemprompt_identifiers::DocumentRevisionId,
    ) -> Result<Projection, DocumentsError> {
        let revision = self
            .repos
            .revisions
            .find(revision)
            .await?
            .ok_or_else(|| DocumentsError::not_found("revision", revision))?;
        Ok(self.projected(document, &revision).await?.ids)
    }

    pub(super) async fn projected(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<Projected, DocumentsError> {
        let bytes = self.revision_bytes(revision).await?;
        Ok(self.projected_from(document, revision, bytes).await?.0)
    }

    pub(super) async fn projected_with_bytes(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<(Projected, Vec<u8>), DocumentsError> {
        let bytes = self.parent_bytes(revision).await?;
        self.projected_from(document, revision, bytes).await
    }

    async fn projected_from(
        &self,
        document: &Document,
        revision: &DocumentRevision,
        bytes: Vec<u8>,
    ) -> Result<(Projected, Vec<u8>), DocumentsError> {
        let paths = self.project_paths(document.format, bytes.clone()).await?;
        let entries = self.node_ids(revision, &paths).await?;
        let projected = Projected {
            ids: with_ids(paths.clone(), &entries),
            paths,
        };
        Ok((projected, bytes))
    }

    async fn node_ids(
        &self,
        revision: &DocumentRevision,
        paths: &Projection,
    ) -> Result<Vec<NodeIdEntry>, DocumentsError> {
        let stored = self.repos.node_ids.list(&revision.id).await?;
        if stored.is_empty() && paths.nodes().next().is_some() {
            return Err(DocumentsError::not_found("revision", &revision.id));
        }
        Ok(stored)
    }

    pub async fn backfill_node_ids(
        &self,
        revision: &DocumentRevision,
    ) -> Result<usize, DocumentsError> {
        let document = self
            .repos
            .documents
            .find(&revision.document_id)
            .await?
            .ok_or_else(|| DocumentsError::not_found("document", &revision.document_id))?;
        let bytes = self.revision_bytes(revision).await?;
        let paths = self.project_paths(document.format, bytes).await?;
        let parent = self.parent_ids(&document, revision).await?;
        let mut tx = self.repos.documents.begin().await?;
        if DocumentNodeIdRepository::lock_revision(&mut tx, &revision.id).await? {
            return Ok(0);
        }
        let writer = writer_tag(revision.id.as_str());
        let count = i64::try_from(paths.nodes().count()).unwrap_or(i64::MAX);
        let first =
            DocumentNodeIdRepository::reserve(&mut tx, &document.id, &writer, count).await?;
        let mut minter = NodeIdMinter::new(writer, u64::try_from(first - 1).unwrap_or(0));
        let entries = blocking(move || {
            Ok(match parent {
                Some(parent) => carry_forward(&parent, &paths, &CarryHints::default(), &mut minter),
                None => mint_all(&paths, &mut minter),
            })
        })
        .await?;
        DocumentNodeIdRepository::insert(&mut tx, &document.id, &revision.id, &entries).await?;
        tx.commit()
            .await
            .map_err(|error| DocumentsError::Repository(RepositoryError::from(error)))?;
        Ok(entries.len())
    }

    async fn parent_ids(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<Option<Projection>, DocumentsError> {
        let Some(parent_id) = revision.parent_revision_id.as_ref() else {
            return Ok(None);
        };
        let stored = self.repos.node_ids.list(parent_id).await?;
        if stored.is_empty() {
            return Ok(None);
        }
        let Some(parent) = self.repos.revisions.find(parent_id).await? else {
            return Ok(None);
        };
        let bytes = self.parent_bytes(&parent).await?;
        let paths = self.project_paths(document.format, bytes).await?;
        Ok(Some(with_ids(paths, &stored)))
    }
}
