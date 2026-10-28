//! The engine seam: the traits a third-party CRDT, formula or rendering
//! engine implements so the files domain never depends on one.
//!
//! `systemprompt-files` names no engine crate. A composition root picks an
//! engine (for example `systemprompt-engine-loro` behind the facade's
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

use std::fmt;

pub use error::EngineError;

use super::model::RichText;

/// Core's engine-agnostic view of a document's editable content.
/// `RichText` is a text document as typed blocks with marks. `Body` is the
/// editable body as one flat text (the inner markup of `<body>` for HTML,
/// the whole text for Markdown) that only sessions opened before the
/// `RichText` model still hold; new sessions never seed it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelView {
    Body(String),
    RichText(RichText),
}

impl ModelView {
    /// The model's wire name, reported to a joining client.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Body(_) => "body",
            Self::RichText(_) => "rich_text",
        }
    }
}

/// A live-session CRDT engine.
///
/// `wire_version` names the wire format a client must also speak. `seed`
/// opens a document from a model, and every replica seeding the same model
/// produces the same operations; `open` reads a snapshot this engine
/// exported; `check_update` refuses a malformed update before it is logged.
pub trait DocumentEngine: Send + Sync + fmt::Debug {
    fn name(&self) -> &'static str;

    fn wire_version(&self) -> &'static str;

    fn seed(&self, model: &ModelView) -> Result<Box<dyn EngineDoc>, EngineError>;

    fn open(&self, snapshot: &[u8]) -> Result<Box<dyn EngineDoc>, EngineError>;

    fn check_update(&self, update: &[u8]) -> Result<(), EngineError>;
}

/// One open document of a [`DocumentEngine`].
///
/// Re-applying an update is a no-op. `version` is the encoded version vector
/// of everything applied; `missing_since` returns what a peer at a version
/// lacks, or `None`. `update_to_model` computes, on a fork, the update that
/// turns this document into a model, or `None` when it already matches.
pub trait EngineDoc: Send + fmt::Debug {
    fn apply_update(&mut self, update: &[u8]) -> Result<(), EngineError>;

    fn snapshot(&self) -> Result<Vec<u8>, EngineError>;

    fn version(&self) -> Vec<u8>;

    fn missing_since(&self, version: &[u8]) -> Result<Option<Vec<u8>>, EngineError>;

    fn export_model(&self) -> ModelView;

    fn fork(&self) -> Box<dyn EngineDoc>;

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError>;
}

/// Recalculates formulas. Without one, cells keep their cached values.
pub trait FormulaEngine: Send + Sync + fmt::Debug {
    fn recalc(&self, model: &ModelView) -> Result<ModelView, EngineError>;
}

/// Renders a document to PDF. Without one, PDF export is refused.
pub trait RenderEngine: Send + Sync + fmt::Debug {
    fn pdf(&self, model: &ModelView) -> Result<Vec<u8>, EngineError>;
}
