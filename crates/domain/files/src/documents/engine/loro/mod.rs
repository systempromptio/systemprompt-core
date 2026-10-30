//! The Loro implementation of the files domain's [`DocumentEngine`], compiled
//! only with the `collab-loro` feature.
//!
//! A document holds the [`RichText`] model in the containers described in
//! [`rich`]. [`LoroEngine::seed`] writes the initial model with peer id
//! [`SEED_PEER`], then moves to a random peer id. The files domain seeds a
//! session once, stores the seed's snapshot and has every replica
//! [`DocumentEngine::open`] that snapshot, so replicas never depend on two
//! seeds of one revision producing identical operations. A session opened
//! before the model existed holds one [`LoroText`] named [`BODY_CONTAINER`]
//! instead and exports as [`ModelView::Body`]. A browser editor mirrors the
//! containers with `loro-crdt` at [`WIRE_VERSION`] and configures
//! [`rich::TEXT_STYLES`].
//!
//! Loro can panic on a hostile blob, and the release profile aborts on
//! panic, so no caller can contain one. Every update is therefore checked by
//! [`check_update_shape`] from its encoded header before any import: its
//! size, its mode, its change, peer and operation counts, the consistency of
//! its version ranges and, for a client update, its peers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod rich;
mod sync;

pub use sync::write_model;

use crate::documents::engine::{DocumentEngine, EngineDoc, EngineError, ModelView, UpdateImport};
use crate::documents::model::RichText;
use loro::{EncodedBlobMode, ExportMode, LoroDoc, LoroText, PeerID, UpdateOptions, VersionVector};

pub const BODY_CONTAINER: &str = "body";
pub const SEED_PEER: PeerID = 0;
pub const WIRE_VERSION: &str = "1.17.1";

#[derive(Debug, Clone, Copy, Default)]
pub struct LoroEngine;

impl DocumentEngine for LoroEngine {
    fn name(&self) -> &'static str {
        "loro"
    }

    fn wire_version(&self) -> &'static str {
        WIRE_VERSION
    }

    fn seed(&self, model: &ModelView) -> Result<Box<dyn EngineDoc>, EngineError> {
        Ok(Box::new(LoroEngineDoc(
            seed_model(model).map_err(EngineError::State)?,
        )))
    }

    fn open(&self, snapshot: &[u8]) -> Result<Box<dyn EngineDoc>, EngineError> {
        let doc = LoroDoc::new();
        rich::configure(&doc);
        doc.import(snapshot).map_err(state)?;
        Ok(Box::new(LoroEngineDoc(doc)))
    }

    fn check_update(&self, update: &[u8], peers: Option<&[u64]>) -> Result<(), EngineError> {
        check_update_shape(update, peers).map_err(|error| match error {
            UpdateShapeError::PeerNotAssigned { peer } => EngineError::PeerNotAssigned { peer },
            other => EngineError::InvalidUpdate(Box::new(other)),
        })
    }
}

/// Why an update was refused from its encoded header, before any import.
#[derive(Debug, thiserror::Error)]
pub enum UpdateShapeError {
    #[error("the update is {len} bytes, over the {max} byte bound")]
    TooLarge { len: usize, max: usize },
    #[error("the update header could not be decoded")]
    Header(#[source] loro::LoroError),
    #[error("the blob is a {mode}, not an incremental update")]
    NotAnUpdate { mode: EncodedBlobMode },
    #[error("the update carries {changes} changes, over the {max} change bound")]
    TooManyChanges { changes: u32, max: u32 },
    #[error("the update spans {peers} peers, over the {max} peer bound")]
    TooManyPeers { peers: usize, max: usize },
    #[error("the update's version range for peer {peer} is inconsistent")]
    InconsistentRange { peer: PeerID },
    #[error("the update spans {ops} operations of peer {peer}, over the {max} bound")]
    RangeTooWide { peer: PeerID, ops: i64, max: i64 },
    #[error("the update carries operations of peer {peer}, which the caller was not assigned")]
    PeerNotAssigned { peer: PeerID },
}

pub const MAX_UPDATE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_UPDATE_CHANGES: u32 = 50_000;
pub const MAX_UPDATE_PEERS: usize = 64;
pub const MAX_UPDATE_PEER_OPS: i64 = 1_000_000;

pub fn check_update_shape(update: &[u8], peers: Option<&[u64]>) -> Result<(), UpdateShapeError> {
    if update.len() > MAX_UPDATE_BYTES {
        return Err(UpdateShapeError::TooLarge {
            len: update.len(),
            max: MAX_UPDATE_BYTES,
        });
    }
    let meta = LoroDoc::decode_import_blob_meta(update, true).map_err(UpdateShapeError::Header)?;
    if !matches!(meta.mode, EncodedBlobMode::Updates) {
        return Err(UpdateShapeError::NotAnUpdate { mode: meta.mode });
    }
    if meta.change_num > MAX_UPDATE_CHANGES {
        return Err(UpdateShapeError::TooManyChanges {
            changes: meta.change_num,
            max: MAX_UPDATE_CHANGES,
        });
    }
    let (start, end) = (&meta.partial_start_vv, &meta.partial_end_vv);
    if end.len() > MAX_UPDATE_PEERS || start.len() > MAX_UPDATE_PEERS {
        return Err(UpdateShapeError::TooManyPeers {
            peers: end.len().max(start.len()),
            max: MAX_UPDATE_PEERS,
        });
    }
    if let Some(peer) = start.keys().find(|peer| !end.contains_key(*peer)) {
        return Err(UpdateShapeError::InconsistentRange { peer: *peer });
    }
    for (peer, last) in end.iter() {
        let first = start.get(peer).copied().unwrap_or(0);
        if first < 0 || *last < first {
            return Err(UpdateShapeError::InconsistentRange { peer: *peer });
        }
        let ops = i64::from(*last) - i64::from(first);
        if ops > MAX_UPDATE_PEER_OPS {
            return Err(UpdateShapeError::RangeTooWide {
                peer: *peer,
                ops,
                max: MAX_UPDATE_PEER_OPS,
            });
        }
        if ops > 0
            && let Some(assigned) = peers
            && !assigned.contains(peer)
        {
            return Err(UpdateShapeError::PeerNotAssigned { peer: *peer });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub struct LoroEngineDoc(pub LoroDoc);

impl EngineDoc for LoroEngineDoc {
    fn apply_update(&mut self, update: &[u8]) -> Result<UpdateImport, EngineError> {
        let status = self.0.import(update).map_err(state)?;
        Ok(UpdateImport {
            peers: status
                .success
                .iter()
                .filter(|(_, (start, end))| end > start)
                .map(|(peer, _)| *peer)
                .collect(),
            pending: status.pending.is_some(),
        })
    }

    fn snapshot(&self) -> Result<Vec<u8>, EngineError> {
        self.0.export(ExportMode::Snapshot).map_err(encode)
    }

    fn version(&self) -> Vec<u8> {
        self.0.oplog_vv().encode()
    }

    fn missing_since(&self, version: &[u8]) -> Result<Option<Vec<u8>>, EngineError> {
        let known =
            VersionVector::decode(version).map_err(|e| EngineError::InvalidVersion(Box::new(e)))?;
        if known.includes_vv(&self.0.oplog_vv()) {
            return Ok(None);
        }
        self.0
            .export(ExportMode::updates(&known))
            .map(Some)
            .map_err(encode)
    }

    fn export_model(&self) -> Result<ModelView, EngineError> {
        export(&self.0)
    }

    fn fork(&self) -> Box<dyn EngineDoc> {
        let fork = self.0.fork();
        rich::configure(&fork);
        Box::new(Self(fork))
    }

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError> {
        if export(&self.0)? == *model {
            return Ok(None);
        }
        let before = self.0.oplog_vv();
        let draft = self.0.fork();
        draft.set_peer_id(self.0.peer_id()).map_err(state)?;
        rich::configure(&draft);
        match model {
            ModelView::RichText(target) => {
                write_model(&draft, target).map_err(EngineError::State)?;
            },
            ModelView::Body(target) => draft
                .get_text(BODY_CONTAINER)
                .update(target, UpdateOptions::default())
                .map_err(|e| EngineError::State(Box::new(e)))?,
        }
        draft.commit();
        draft
            .export(ExportMode::updates(&before))
            .map(Some)
            .map_err(encode)
    }
}

pub fn export(doc: &LoroDoc) -> Result<ModelView, EngineError> {
    let body = body_text(doc);
    if rich::blocks(doc).is_empty() && !body.is_empty() {
        Ok(ModelView::Body(body))
    } else {
        Ok(ModelView::RichText(
            rich::read(doc).map_err(|e| EngineError::Model(Box::new(e)))?,
        ))
    }
}

#[must_use]
pub fn body_text(doc: &LoroDoc) -> String {
    doc.get_text(BODY_CONTAINER).to_string()
}

pub fn seed_model(model: &ModelView) -> Result<LoroDoc, rich::WriteError> {
    let doc = LoroDoc::new();
    rich::configure(&doc);
    doc.set_peer_id(SEED_PEER)?;
    match model {
        ModelView::RichText(rich) => write_model(&doc, rich)?,
        ModelView::Body(body) => {
            let text: LoroText = doc.get_text(BODY_CONTAINER);
            text.insert(0, body)?;
        },
    }
    doc.commit();
    doc.set_peer_id(replica_peer())?;
    Ok(doc)
}

pub fn seed(body: &str) -> Result<LoroDoc, rich::WriteError> {
    seed_model(&ModelView::Body(body.to_owned()))
}

pub fn seed_rich(model: &RichText) -> Result<LoroDoc, rich::WriteError> {
    seed_model(&ModelView::RichText(model.clone()))
}

#[must_use]
pub fn replica_peer() -> PeerID {
    uuid::Uuid::new_v4().as_u64_pair().0.max(1)
}

fn state(error: loro::LoroError) -> EngineError {
    EngineError::State(Box::new(error))
}

fn encode(error: loro::LoroEncodeError) -> EngineError {
    EngineError::Encode(Box::new(error))
}
