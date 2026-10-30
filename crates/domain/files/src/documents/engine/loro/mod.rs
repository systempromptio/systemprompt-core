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
//! A table opened from a snapshot written before tables were containers
//! still keeps its cells in `kind`; [`EngineDoc::update_to_model`] of such a
//! document never answers `None`, so writing its own model back upgrades it
//! (see [`table`]). [`edit`] edits blocks, rows and cells by id, and
//! [`scope`] reads which blocks an import touched.
//!
//! Loro can panic on a hostile blob, and the release profile aborts on
//! panic, so no caller can contain one. Every update is therefore checked by
//! [`check_update_shape`] from its encoded header before any import: its
//! size, its mode, its change, peer and operation counts, the consistency of
//! its version ranges and, for a client update, its peers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod deck;
pub mod edit;
mod edit_table;
pub mod rich;
pub mod scope;
mod shape;
mod shape_ops;
pub mod sheet;
pub(crate) mod structured;
mod sync;
pub mod table;
mod write;

pub use shape::{
    MAX_UPDATE_BYTES, MAX_UPDATE_CHANGES, MAX_UPDATE_PEER_OPS, MAX_UPDATE_PEERS, UpdateShapeError,
    check_update_shape,
};

pub use sync::write_model;

use crate::documents::engine::{
    DocumentEngine, EngineDoc, EngineError, ImportScope, ModelView, ScopedImport, UpdateImport,
};
use crate::documents::model::RichText;
use loro::{ExportMode, LoroDoc, LoroText, PeerID, UpdateOptions, VersionVector};

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

    fn apply_update_scoped(
        &mut self,
        update: &[u8],
        scoped: bool,
    ) -> Result<ScopedImport, EngineError> {
        let before = self.0.state_frontiers();
        let scoped = scoped && !structured::is_structured(&self.0);
        let before_model = if scoped { None } else { Some(export(&self.0)?) };
        let import = self.apply_update(update)?;
        let after = self.0.state_frontiers();
        let before_model = match before_model {
            Some(model) => model,
            None => {
                if let Some(touched) = scope::touched(&self.0, &before, &after) {
                    return Ok(ScopedImport {
                        import,
                        scope: ImportScope::Blocks(touched),
                    });
                }
                export(&self.0.fork_at(&before).map_err(state)?)?
            },
        };
        let after = export(&self.0)?;
        structured::check(&self.0, &before_model, &after)?;
        Ok(ScopedImport {
            import,
            scope: ImportScope::Full {
                before: before_model,
                after,
            },
        })
    }

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError> {
        self.update_to_model_as(model, self.0.peer_id())
    }

    fn update_from_ops(
        &self,
        ops: &[systemprompt_models::documents::EditOp],
        peer: u64,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        shape_ops::update_from_ops(&self.0, ops, peer)
    }

    fn update_to_model_as(
        &self,
        model: &ModelView,
        peer: u64,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        let model = &canonical_view(model);
        if !rich::has_legacy_tables(&self.0) && export(&self.0)? == *model {
            return Ok(None);
        }
        let before = self.0.oplog_vv();
        let draft = self.0.fork();
        draft.set_peer_id(peer).map_err(state)?;
        rich::configure(&draft);
        match model {
            ModelView::RichText(target) => {
                write_model(&draft, target).map_err(EngineError::State)?;
            },
            ModelView::Sheet(target) => {
                sheet::write_sheets(&draft, target).map_err(EngineError::State)?;
            },
            ModelView::Deck(target) => {
                deck::write_deck(&draft, target).map_err(EngineError::State)?;
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

fn canonical_view(model: &ModelView) -> ModelView {
    let mut model = model.clone();
    if let ModelView::RichText(rich) = &mut model {
        table::canonical(rich);
    }
    model
}

pub fn export(doc: &LoroDoc) -> Result<ModelView, EngineError> {
    if sheet::is_sheet_doc(doc) {
        return sheet::read(doc)
            .map(ModelView::Sheet)
            .map_err(|e| EngineError::Model(Box::new(e)));
    }
    if !deck::slides(doc).is_empty() {
        return deck::read(doc)
            .map(ModelView::Deck)
            .map_err(|e| EngineError::Model(Box::new(e)));
    }
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
        ModelView::Sheet(sheets) => sheet::write_sheets(&doc, sheets)?,
        ModelView::Deck(slides) => deck::write_deck(&doc, slides)?,
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

pub(crate) fn state(error: loro::LoroError) -> EngineError {
    EngineError::State(Box::new(error))
}

pub(crate) fn encode(error: loro::LoroEncodeError) -> EngineError {
    EngineError::Encode(Box::new(error))
}
