//! Reading a document's live session: [`DocumentService::live_read`] and
//! the live projection a read answers while a session is open.
//!
//! The session's live bytes are projected with the node ids of its base
//! revision carried onto them, every node edited in place keeping its base
//! id (the id its checkpoint keeps too); a text session then pins each
//! block to the id its live model gave it, so only nodes no model block
//! names (a Markdown document's source lines) rely on the carry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, Projection};

use super::DocumentService;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::nodes::{CarryHints, NodeIdMinter, carry_forward, with_ids, writer_tag};

const LIVE_WRITER: &str = "live-ops";

impl DocumentService {
    pub async fn live_read(
        &self,
        who: &AuthenticatedUser,
        document_id: &DocumentId,
        trace: &TraceId,
    ) -> Result<Option<(DocumentRevision, Projection, Vec<u8>)>, DocumentsError> {
        let document = self
            .authorized_document(who, document_id, DocumentAction::View, trace)
            .await?;
        self.live_projected(&document).await
    }

    #[cfg(feature = "collab")]
    pub(crate) async fn revision_ids(
        &self,
        document: &Document,
        revision: &DocumentRevisionId,
    ) -> Result<Projection, DocumentsError> {
        let revision = self
            .repos
            .revisions
            .find(revision)
            .await?
            .ok_or_else(|| DocumentsError::not_found("revision", revision))?;
        Ok(self.projected(document, &revision).await?.ids)
    }

    #[cfg(feature = "collab")]
    pub(super) async fn live_projection(
        &self,
        document: &Document,
    ) -> Result<Option<(DocumentRevision, Projection)>, DocumentsError> {
        Ok(self
            .live_projected(document)
            .await?
            .map(|(base, projection, _)| (base, projection)))
    }

    #[cfg(not(feature = "collab"))]
    #[expect(
        clippy::unused_async,
        reason = "shares the signature of the collab build"
    )]
    async fn live_projected(
        &self,
        _document: &Document,
    ) -> Result<Option<(DocumentRevision, Projection, Vec<u8>)>, DocumentsError> {
        Ok(None)
    }

    #[cfg(feature = "collab")]
    async fn live_projected(
        &self,
        document: &Document,
    ) -> Result<Option<(DocumentRevision, Projection, Vec<u8>)>, DocumentsError> {
        use std::collections::HashSet;

        use systemprompt_identifiers::DocumentNodeId;
        let Some(collab) = self.collab() else {
            return Ok(None);
        };
        let view = collab
            .live_view(document)
            .await
            .map_err(|error| DocumentsError::LiveSession(Box::new(error)))?;
        let Some(view) = view else {
            return Ok(None);
        };
        let base = self
            .repos
            .revisions
            .find(&view.base)
            .await?
            .ok_or_else(|| DocumentsError::not_found("revision", &view.base))?;
        let base_ids = self.projected(document, &base).await?.ids;
        let bytes = view.bytes;
        let paths = self.project_paths(document.format, bytes.clone()).await?;
        let hints = CarryHints {
            rewritten: base_ids.nodes().map(|node| node.id.clone()).collect(),
            ..CarryHints::default()
        };
        let pins = view
            .model
            .as_ref()
            .map(|model| super::live_ids::pinned_ids(&paths, model))
            .unwrap_or_default();
        let writer = writer_tag(&format!("{LIVE_WRITER}:{}", document.id));
        let projected = paths.clone();
        let entries = blocking(move || {
            let mut minter = NodeIdMinter::new(writer, 0);
            let mut entries = carry_forward(&base_ids, &projected, &hints, &mut minter);
            let pinned: HashSet<&DocumentNodeId> = pins.values().collect();
            for entry in &mut entries {
                match pins.get(entry.path.as_str()) {
                    Some(id) => entry.node_id = id.clone(),
                    None if pinned.contains(&entry.node_id) => entry.node_id = minter.mint(),
                    None => {},
                }
            }
            Ok(entries)
        })
        .await?;
        Ok(Some((base, with_ids(paths, &entries), bytes)))
    }
}
