//! Stable node ids of revisions: read from the store, carried forward from
//! the parent revision, or minted.
//!
//! A commit records the new revision's ids in the transaction that moves
//! its head, so a head always has ids. A revision written before ids existed
//! gets them on first projection, carried from its parent when the parent
//! has ids, in a transaction that locks the revision row: a second reader
//! minting it at once waits, then reads the first reader's ids.
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

    pub(super) async fn projected(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<Projected, DocumentsError> {
        Ok(self.projected_with_bytes(document, revision).await?.0)
    }

    pub(super) async fn projected_with_bytes(
        &self,
        document: &Document,
        revision: &DocumentRevision,
    ) -> Result<(Projected, Vec<u8>), DocumentsError> {
        let bytes = self.revision_bytes(revision).await?;
        let paths = self.project_paths(document.format, bytes.clone()).await?;
        let entries = self.node_ids(document, revision, &paths).await?;
        let projected = Projected {
            ids: with_ids(paths.clone(), &entries),
            paths,
        };
        Ok((projected, bytes))
    }

    async fn node_ids(
        &self,
        document: &Document,
        revision: &DocumentRevision,
        paths: &Projection,
    ) -> Result<Vec<NodeIdEntry>, DocumentsError> {
        let stored = self.repos.node_ids.list(&revision.id).await?;
        if !stored.is_empty() {
            return Ok(stored);
        }
        let parent = self.parent_ids(document, revision).await?;
        let mut tx = self.repos.documents.begin().await?;
        if DocumentNodeIdRepository::lock_revision(&mut tx, &revision.id).await? {
            drop(tx);
            return Ok(self.repos.node_ids.list(&revision.id).await?);
        }
        let writer = writer_tag(revision.id.as_str());
        let count = i64::try_from(paths.nodes().count()).unwrap_or(i64::MAX);
        let first =
            DocumentNodeIdRepository::reserve(&mut tx, &document.id, &writer, count).await?;
        let mut minter = NodeIdMinter::new(writer, u64::try_from(first - 1).unwrap_or(0));
        let paths = paths.clone();
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
        Ok(entries)
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
        let bytes = self.revision_bytes(&parent).await?;
        let paths = self.project_paths(document.format, bytes).await?;
        Ok(Some(with_ids(paths, &stored)))
    }
}
