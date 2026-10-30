//! Applying edit ops to a live session's tree instead of the head.
//!
//! [`DocumentService::admit_live_ops`] runs the checks a commit runs before
//! it builds anything (edit access, input limits, agent write rules), so an
//! op batch that would be refused as a commit is refused live too.
//! [`DocumentService::apply_ops_live`] then applies the batch to the bytes
//! the live model materialises to: the node ids of the session's base
//! revision are carried onto that projection, so a guard names a node the
//! caller read and is checked against the text everyone sees now, not the
//! text the head held. A node the live session no longer has is an unknown
//! node; one whose text changed is a stale anchor carrying its live text.
//! The projections before and after are returned so the caller can answer
//! with the lines it wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentAction, EditOp, Projection};

use super::DocumentService;
use super::apply::{HeadEdit, apply_on_head};
use super::node_ids::Projected;
use super::support::blocking;
use crate::documents::error::DocumentsError;
use crate::documents::model::Media;
use crate::documents::models::{Document, DocumentRevision};
use crate::documents::nodes::{CarryHints, NodeIdMinter, carry_forward, with_ids, writer_tag};
use crate::documents::revision::CommitBody;

const LIVE_WRITER: &str = "live-ops";

/// An op batch for the live session of `document_id`, with the media its
/// image ops reference already resolved.
#[derive(Debug)]
pub struct LiveEditRequest {
    pub document_id: DocumentId,
    pub ops: Vec<EditOp>,
    pub media: Media,
}

/// An op batch written into a live session.
///
/// It carries the update's sequence number
/// (`None` when the batch changed nothing), the revision the session is
/// based on, and the live tree before and after, so the caller answers with
/// the lines it wrote. A batch the engine translated itself has empty trees
/// and instead names the block and cell ids it wrote in `touched`.
#[derive(Debug, Clone)]
pub struct LiveEdit {
    pub session_id: DocumentEditSessionId,
    pub seq: Option<i64>,
    pub base_revision: DocumentRevisionId,
    pub before: Projection,
    pub after: Projection,
    pub applied: usize,
    pub touched: Vec<String>,
}

/// The outcome of [`DocumentService::apply_ops_live`]: the edited bytes and
/// the id-addressed projections of the live tree before and after.
#[derive(Debug, Clone)]
pub struct LiveApplied {
    pub bytes: Vec<u8>,
    pub before: Projection,
    pub after: Projection,
}

/// One op batch applied to the live tree of `document`, whose session is
/// based on `base` and currently materialises to `live`.
#[derive(Debug)]
pub struct LiveOps<'a> {
    pub document: &'a Document,
    pub base: &'a DocumentRevisionId,
    pub live: Vec<u8>,
    pub ops: Vec<EditOp>,
    pub media: Media,
}

impl DocumentService {
    pub async fn admit_live_ops(
        &self,
        who: &AuthenticatedUser,
        document: &DocumentId,
        ops: &[EditOp],
        trace: &TraceId,
    ) -> Result<Document, DocumentsError> {
        let document = self
            .authorized_document(who, document, DocumentAction::Edit, trace)
            .await?;
        let body = CommitBody::Ops(ops.to_vec());
        self.check_body_limits(&body)?;
        self.agent_write_rules(who, &document, &body).await?;
        Ok(document)
    }

    pub async fn apply_ops_live(
        &self,
        request: LiveOps<'_>,
    ) -> Result<LiveApplied, DocumentsError> {
        let LiveOps {
            document,
            base,
            live,
            ops,
            media,
        } = request;
        let base_revision = self
            .repos
            .revisions
            .find(base)
            .await?
            .ok_or_else(|| DocumentsError::not_found("revision", base))?;
        let base_ids = self.projected(document, &base_revision).await?.ids;
        let before = self.carried(document, &base_ids, live.clone()).await?;
        let codec = self.codec(document.format)?;
        let format = document.format;
        let head = before.clone();
        let applied = blocking(move || {
            apply_on_head(
                codec.as_ref(),
                format,
                &live,
                &head,
                HeadEdit {
                    ops,
                    fragments: Vec::new(),
                    media,
                },
            )
        })
        .await?;
        let after = self
            .carried(document, &before.ids, applied.bytes.clone())
            .await?;
        Ok(LiveApplied {
            bytes: applied.bytes,
            before: before.ids,
            after: after.ids,
        })
    }

    async fn carried(
        &self,
        document: &Document,
        previous: &Projection,
        bytes: Vec<u8>,
    ) -> Result<Projected, DocumentsError> {
        let paths = self.project_paths(document.format, bytes).await?;
        let previous = previous.clone();
        let writer = writer_tag(&format!("{LIVE_WRITER}:{}", document.id));
        let projected = paths.clone();
        let entries = blocking(move || {
            let mut minter = NodeIdMinter::new(writer, 0);
            Ok(carry_forward(
                &previous,
                &projected,
                &CarryHints::default(),
                &mut minter,
            ))
        })
        .await?;
        Ok(Projected {
            ids: with_ids(paths.clone(), &entries),
            paths,
        })
    }

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
        if self
            .repos
            .edit_sessions
            .find_open(&document.id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
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
            rewritten: if view.model.is_none() {
                base_ids.nodes().map(|node| node.id.clone()).collect()
            } else {
                HashSet::new()
            },
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
