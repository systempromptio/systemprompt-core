//! Stable node ids of revisions: read from the store, carried forward from
//! the parent revision, or minted.
//!
//! A commit records the new revision's ids before its head moves, so a head
//! always has ids. A revision written before ids existed gets them on first
//! projection, carried from its parent when the parent has ids; its writer
//! tag derives from the revision id, so two readers minting it at once
//! compute the same ids and the second insert is a no-op.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_models::documents::{DocumentFormat, Projection};

use super::DocumentService;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision, NodeIdEntry};
use crate::documents::nodes::{NodeIdMinter, carry_forward, mint_all, with_ids, writer_tag};

pub(super) struct IdSources<'a> {
    pub(super) previous: Option<&'a Projection>,
    pub(super) next: &'a Projection,
    pub(super) retired: &'a HashSet<DocumentNodeId>,
}

#[derive(Debug, Clone)]
pub(super) struct Projected {
    pub paths: Projection,
    pub ids: Projection,
}

fn writer_key(revision: &DocumentRevision) -> String {
    revision
        .attribution
        .session_id
        .as_ref()
        .map(ToString::to_string)
        .or_else(|| revision.origin_id.clone())
        .unwrap_or_else(|| revision.id.to_string())
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
        let writer = writer_tag(revision.id.as_str());
        let mut minter = self.minter(document, writer).await?;
        let entries = match self.parent_ids(document, revision).await? {
            Some(parent) => carry_forward(&parent, paths, &HashSet::new(), &mut minter),
            None => mint_all(paths, &mut minter),
        };
        self.repos
            .node_ids
            .insert(&document.id, &revision.id, &entries)
            .await?;
        Ok(self.repos.node_ids.list(&revision.id).await?)
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

    async fn minter(
        &self,
        document: &Document,
        writer: String,
    ) -> Result<NodeIdMinter, DocumentsError> {
        let last = self
            .repos
            .node_ids
            .max_counter(&document.id, &writer)
            .await?;
        Ok(NodeIdMinter::new(writer, u64::try_from(last).unwrap_or(0)))
    }

    pub(super) async fn record_ids(
        &self,
        document: &Document,
        revision: &DocumentRevision,
        IdSources {
            previous,
            next,
            retired,
        }: IdSources<'_>,
    ) -> Result<(), DocumentsError> {
        let mut minter = self
            .minter(document, writer_tag(&writer_key(revision)))
            .await?;
        let entries = match previous {
            Some(previous) => carry_forward(previous, next, retired, &mut minter),
            None => mint_all(next, &mut minter),
        };
        self.repos
            .node_ids
            .insert(&document.id, &revision.id, &entries)
            .await?;
        Ok(())
    }
}
