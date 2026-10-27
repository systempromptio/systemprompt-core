//! Shared document store seams: document codecs and template providers.
//!
//! A [`DocumentCodec`] turns the bytes of one document format into a
//! [`Projection`] — a tree of addressable nodes, each carrying a stable
//! [`DocumentNodeId`](systemprompt_identifiers::DocumentNodeId) and an
//! eight-hex-character content hash — and applies [`EditOp`]s back onto
//! those bytes. Agents read a projection as hashline text
//! (`<id>:<hash8>| <text>`) and quote the hash back on every edit; a codec
//! rejects an edit whose hash no longer matches with
//! [`CodecError::StaleAnchor`], so an agent never overwrites text it has not
//! seen.
//!
//! A [`DocumentTemplateProvider`] contributes the templates a new document
//! can be instantiated from.
//!
//! Both seams are implemented by extensions and registered at compile time
//! with [`register_document_codec!`](crate::register_document_codec) and
//! [`register_document_template_provider!
//! `](crate::register_document_template_provider). Every implementation reports
//! a `priority` (default 100); when several implementations cover the same
//! format or template id, the highest priority wins, and [`registered_codecs`]
//! and [`registered_template_providers`] yield implementations highest priority
//! first.
//!
//! The format, projection and edit-op vocabulary lives here rather than in
//! `systemprompt-models` because the codec trait names it and
//! `systemprompt-models` depends on this crate; `systemprompt-models`
//! re-exports it under `documents`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod codec;
mod edit_op;
mod format;
mod projection;
mod target;
mod template;

pub use codec::{
    CodecError, DocumentCodec, DocumentCodecRegistration, NodeFragment, TemplateParams, codec_for,
    fragment_insert_op, registered_codecs, text_fragment,
};
pub use edit_op::{CellValue, ContentAs, EditOp, NodeTarget, QuoteTarget, required_hash};
pub use format::{DocumentFormat, UnknownDocumentFormat};
pub use projection::{NodeKind, Projection, ProjectionNode, content_hash, hash8};
pub use target::{ClosestBlock, READ_ONLY_ATTR, ResolvedBatch, TargetError, resolve_targets};
pub use template::{
    DocumentTemplate, DocumentTemplateProvider, DocumentTemplateProviderRegistration,
    TemplatePlaceholder, TemplateSource, registered_template_providers,
};
