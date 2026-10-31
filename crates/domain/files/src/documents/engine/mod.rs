//! The engine seam: the traits a third-party CRDT, formula or rendering
//! engine implements so the files domain never depends on one.
//!
//! `systemprompt-files` names no engine crate. A composition root picks an
//! engine (for example the `loro` module behind the facade's
//! `files-collab-loro` feature) and hands it to the service that needs it;
//! with none selected, live sessions answer `collab_engine_unavailable`,
//! formulas keep their cached values and PDF export is refused.
//!
//! Every engine speaks [`ModelView`], core's own engine-agnostic view of a
//! document's editable content, so codecs convert native files to and from
//! a `ModelView` and never to or from an engine's containers. Swapping an
//! engine touches only the engine crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;

#[cfg(feature = "collab-loro")]
pub mod loro;

use std::fmt;

pub use error::EngineError;

use super::model::RichText;

/// Core's engine-agnostic view of a document's editable content.
///
/// `RichText` is a text document as typed blocks with marks. `Body` is the
/// editable body as one flat text (the inner markup of `<body>` for HTML,
/// the whole text for Markdown) that only sessions opened before the
/// `RichText` model still hold; new sessions never seed it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelView {
    Body(String),
    RichText(RichText),
    #[cfg(feature = "collab-loro")]
    Sheet(loro::sheet::SheetModel),
    #[cfg(feature = "collab-loro")]
    Deck(loro::deck::DeckModel),
}

impl ModelView {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Body(_) => "body",
            Self::RichText(_) => "rich_text",
            #[cfg(feature = "collab-loro")]
            Self::Sheet(_) => "sheet",
            #[cfg(feature = "collab-loro")]
            Self::Deck(_) => "deck",
        }
    }
}

/// A live-session CRDT engine.
///
/// `wire_version` names the wire format a client must also speak. `seed`
/// opens a document from a model, and every replica seeding the same model
/// produces the same operations; `open` reads a snapshot this engine
/// exported; `check_update` refuses a malformed update from its encoded
/// header alone, before any engine import: an update that is not an
/// incremental update, exceeds the engine's size, change, peer or operation
/// bounds, carries inconsistent version ranges, or (when `peers` is given)
/// adds operations of a peer outside `peers`. Callers rely on it as the
/// guard against blobs that would crash the engine, since an engine panic
/// aborts the process rather than unwinding.
pub trait DocumentEngine: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;

    fn wire_version(&self) -> &'static str;

    fn seed(&self, model: &ModelView) -> Result<Box<dyn EngineDoc>, EngineError>;

    fn open(&self, snapshot: &[u8]) -> Result<Box<dyn EngineDoc>, EngineError>;

    fn check_update(&self, update: &[u8], peers: Option<&[u64]>) -> Result<(), EngineError>;
}

/// What importing one update added to an [`EngineDoc`].
///
/// `peers` are the engine peers whose operations the update added, so a
/// caller can bind an untrusted update to the peers its author was assigned.
/// `pending` is set when part of the update depends on operations the
/// document does not hold and was parked rather than applied.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateImport {
    pub peers: Vec<u64>,
    pub pending: bool,
}

/// An op batch translated into one engine update, with the block and cell
/// ids it wrote and, for a workbook, the input each written cell now holds
/// (so a cell past the projection's cell cap still has a line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsUpdate {
    pub update: Vec<u8>,
    pub touched: Vec<String>,
    pub values: Vec<(String, String)>,
}

/// What one imported update changed.
///
/// A caller validates in proportion to it. `Blocks` holds only the blocks the
/// update touched (with the document settings when it changed them); `Full`
/// holds the whole model before and after, when the engine cannot or was not
/// asked to scope it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportScope {
    Blocks(RichText),
    Full { before: ModelView, after: ModelView },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedImport {
    pub import: UpdateImport,
    pub scope: ImportScope,
}

/// One open document of a [`DocumentEngine`].
///
/// Re-applying an update is a no-op. `version` is the encoded version vector
/// of everything applied; `missing_since` returns what a peer at a version
/// lacks, or `None`. `update_to_model` computes, on a fork, the update that
/// turns this document into a model, or `None` when it already matches.
pub trait EngineDoc: Send + fmt::Debug {
    fn apply_update(&mut self, update: &[u8]) -> Result<UpdateImport, EngineError>;

    fn snapshot(&self) -> Result<Vec<u8>, EngineError>;

    fn version(&self) -> Vec<u8>;

    fn missing_since(&self, version: &[u8]) -> Result<Option<Vec<u8>>, EngineError>;

    fn export_model(&self) -> Result<ModelView, EngineError>;

    fn fork(&self) -> Box<dyn EngineDoc>;

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError>;

    fn update_to_model_as(
        &self,
        model: &ModelView,
        peer: u64,
    ) -> Result<Option<Vec<u8>>, EngineError>;

    fn update_from_ops(
        &self,
        _ops: &[systemprompt_models::documents::EditOp],
        _peer: u64,
    ) -> Result<Option<OpsUpdate>, EngineError> {
        Err(EngineError::Unsupported)
    }

    fn adopt_node_ids(
        &mut self,
        _nodes: &[systemprompt_provider_contracts::documents::ProjectionNode],
    ) -> Result<bool, EngineError> {
        Ok(false)
    }

    fn apply_update_scoped(
        &mut self,
        update: &[u8],
        _scoped: bool,
    ) -> Result<ScopedImport, EngineError> {
        let before = self.export_model()?;
        let import = self.apply_update(update)?;
        let after = self.export_model()?;
        Ok(ScopedImport {
            import,
            scope: ImportScope::Full { before, after },
        })
    }
}

/// Recalculates formulas. Without one, cells keep their cached values.
pub trait FormulaEngine: Send + Sync + fmt::Debug {
    fn recalc(&self, model: &ModelView) -> Result<ModelView, EngineError>;
}

/// Renders a document to PDF. Without one, PDF export is refused.
pub trait RenderEngine: Send + Sync + fmt::Debug {
    fn pdf(&self, model: &ModelView) -> Result<Vec<u8>, EngineError>;
}
