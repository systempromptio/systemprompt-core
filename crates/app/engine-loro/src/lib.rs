//! The Loro implementation of the files domain's [`DocumentEngine`].
//!
//! A document holds the [`RichText`] model in the containers described in
//! [`rich`]. [`LoroEngine::seed`] writes the initial model with peer id
//! [`SEED_PEER`], then moves to a random peer id, so every replica that seeds
//! from the same revision produces the same operations and can import any
//! client's updates without a shared snapshot. A session opened before the
//! model existed holds one [`LoroText`] named [`BODY_CONTAINER`] instead and
//! exports as [`ModelView::Body`]. A browser editor mirrors the containers
//! with `loro-crdt` at [`WIRE_VERSION`] and configures [`rich::TEXT_STYLES`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod rich;
mod sync;

pub use sync::write_model;

use loro::{ExportMode, LoroDoc, LoroText, PeerID, UpdateOptions, VersionVector};
use systemprompt_files::documents::engine::{DocumentEngine, EngineDoc, EngineError, ModelView};
use systemprompt_files::documents::model::RichText;

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
        export(&self.0)
    }

    fn fork(&self) -> Box<dyn EngineDoc> {
        let fork = self.0.fork();
        rich::configure(&fork);
        Box::new(Self(fork))
    }

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError> {
        if export(&self.0) == *model {
            return Ok(None);
        }
        let before = self.0.oplog_vv();
        let draft = self.0.fork();
        rich::configure(&draft);
        match model {
            ModelView::RichText(target) => {
                write_model(&draft, target).map_err(EngineError::State)?;
            },
            ModelView::Body(target) => draft
                .get_text(BODY_CONTAINER)
                .update(target, UpdateOptions::default())
                .map_err(|e| EngineError::State(Box::new(e)))?,
            _ => return Err(EngineError::Unsupported),
        }
        draft.commit();
        draft
            .export(ExportMode::updates(&before))
            .map(Some)
            .map_err(encode)
    }
}

#[must_use]
pub fn export(doc: &LoroDoc) -> ModelView {
    let body = body_text(doc);
    if rich::blocks(doc).is_empty() && !body.is_empty() {
        ModelView::Body(body)
    } else {
        ModelView::RichText(rich::read(doc))
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
        _ => return Err(Box::new(loro::LoroError::NotImplemented("model kind"))),
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
