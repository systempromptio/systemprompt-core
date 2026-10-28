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

/// Core's engine-agnostic view of a document's editable content.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelView {
    /// The editable body as one text: the inner markup of `<body>` for
    /// HTML, the whole text for Markdown.
    Body(String),
}

impl ModelView {
    #[must_use]
    pub fn body(&self) -> &str {
        match self {
            Self::Body(text) => text,
        }
    }
}

/// A live-session CRDT engine.
pub trait DocumentEngine: Send + Sync + fmt::Debug {
    /// The engine's name, for logs and the join response.
    fn name(&self) -> &'static str;

    /// The version of the engine's wire format; a client must speak the
    /// same one.
    fn wire_version(&self) -> &'static str;

    /// Opens a document seeded from `model`. Every replica seeding the same
    /// model must produce the same operations.
    fn seed(&self, model: &ModelView) -> Result<Box<dyn EngineDoc>, EngineError>;

    /// Opens a document from a snapshot this engine exported.
    fn open(&self, snapshot: &[u8]) -> Result<Box<dyn EngineDoc>, EngineError>;

    /// Checks that `update` is a well-formed update of this engine before it
    /// is logged.
    fn check_update(&self, update: &[u8]) -> Result<(), EngineError>;
}

/// One open document of a [`DocumentEngine`].
pub trait EngineDoc: Send + fmt::Debug {
    /// Imports an update; importing one already applied is a no-op.
    fn apply_update(&mut self, update: &[u8]) -> Result<(), EngineError>;

    fn snapshot(&self) -> Result<Vec<u8>, EngineError>;

    /// The encoded version vector of everything applied so far.
    fn version(&self) -> Vec<u8>;

    /// The updates a peer at `version` lacks, or `None` when it has them all.
    fn missing_since(&self, version: &[u8]) -> Result<Option<Vec<u8>>, EngineError>;

    fn export_model(&self) -> ModelView;

    /// The update that turns this document into `model`, computed on a fork
    /// so the document itself is unchanged; `None` when it already matches.
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
