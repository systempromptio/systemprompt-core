//! Core's engine-agnostic document models and their codecs.
//!
//! A model is the editable content of a document as core's own Rust types,
//! independent of any CRDT: an engine maps a model to its containers and
//! back, and a codec maps native bytes to a model and back. Swapping an
//! engine never touches a codec.
//!
//! [`RichText`] is the model of text documents. HTML and Markdown import and
//! materialise here; the `docx` and `odt` codecs cut their bodies into
//! [`office`] segments; [`formats`] picks the import and materialise of a
//! format, and [`ops`] applies the rich-text and table edit ops to a model.
//! Every live
//! update is checked with [`validate`] on the server's own fork before it
//! is logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod formats;
pub mod html;
pub mod html_body;
mod html_write;
mod ids;
mod inline;
pub mod markdown;
pub mod office;
pub mod ops;
pub mod document;
pub mod layout;
mod mark_kind;
mod rich_text;
mod validate;
mod view;

pub use ids::carry_ids;
pub use inline::segments;
pub use document::DocumentSettings;
pub use mark_kind::FIELD_CODES;
pub use rich_text::{Block, BlockKind, Mark, MarkKind, RICH_TEXT_VERSION, RichText};
pub use validate::{EditLevel, Limits, ModelRejection, validate};
pub use view::{ModelViewV2, model_view_schema};

#[must_use]
pub fn rich_text_schema() -> schemars::Schema {
    schemars::schema_for!(RichText)
}
