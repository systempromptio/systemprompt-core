//! The Loro implementation of the files domain's [`DocumentEngine`].
//!
//! A document holds one [`LoroText`] container named [`BODY_CONTAINER`]
//! whose value is [`ModelView::Body`]. [`LoroEngine::seed`] writes the
//! initial body with peer id [`SEED_PEER`] and timestamps off, then moves to
//! a random peer id, so every replica that seeds from the same revision
//! produces the same operations and can import any client's updates without
//! a shared snapshot. A browser editor mirrors the same container with
//! `loro-crdt` at [`WIRE_VERSION`], the `loro` crate version this engine is
//! pinned to.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{ExportMode, LoroDoc, LoroText, PeerID, UpdateOptions, VersionVector};
use systemprompt_files::documents::engine::{DocumentEngine, EngineDoc, EngineError, ModelView};

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
        Ok(Box::new(LoroEngineDoc(seed(model.body()).map_err(state)?)))
    }

    fn open(&self, snapshot: &[u8]) -> Result<Box<dyn EngineDoc>, EngineError> {
        let doc = LoroDoc::new();
        doc.import(snapshot).map_err(state)?;
        Ok(Box::new(LoroEngineDoc(doc)))
    }

    fn check_update(&self, update: &[u8]) -> Result<(), EngineError> {
        LoroDoc::decode_import_blob_meta(update, true)
            .map(drop)
            .map_err(|e| EngineError::InvalidUpdate(Box::new(e)))
    }
}

#[derive(Debug)]
pub struct LoroEngineDoc(pub LoroDoc);

impl EngineDoc for LoroEngineDoc {
    fn apply_update(&mut self, update: &[u8]) -> Result<(), EngineError> {
        self.0.import(update).map(drop).map_err(state)
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

    fn export_model(&self) -> ModelView {
        ModelView::Body(body_text(&self.0))
    }

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError> {
        let target = model.body();
        if body_text(&self.0) == target {
            return Ok(None);
        }
        let before = self.0.oplog_vv();
        let draft = self.0.fork();
        draft
            .get_text(BODY_CONTAINER)
            .update(target, UpdateOptions::default())
            .map_err(|e| EngineError::State(Box::new(e)))?;
        draft.commit();
        draft
            .export(ExportMode::updates(&before))
            .map(Some)
            .map_err(encode)
    }
}

#[must_use]
pub fn body_text(doc: &LoroDoc) -> String {
    doc.get_text(BODY_CONTAINER).to_string()
}

pub fn seed(body: &str) -> Result<LoroDoc, loro::LoroError> {
    let doc = LoroDoc::new();
    doc.set_peer_id(SEED_PEER)?;
    let text: LoroText = doc.get_text(BODY_CONTAINER);
    text.insert(0, body)?;
    doc.commit();
    doc.set_peer_id(replica_peer())?;
    Ok(doc)
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
