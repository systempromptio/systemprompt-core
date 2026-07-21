//! Concrete [`UiRenderer`](super::UiRenderer) implementations, one per
//! artifact type.
//!
//! Between them they cover every variant of
//! [`CliArtifact`](systemprompt_models::artifacts::CliArtifact) plus `form`,
//! a coverage guarantee enforced by the registry's exhaustiveness test.
//! [`html`] provides the shared HTML-escaping and element helpers, and
//! [`typed`] the schema-faithful payload decoding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod card;
mod chart;
mod dashboard;
mod form;
mod form_field;
pub mod html;
mod image;
mod list;
mod media;
mod message;
mod table;
mod text;
mod typed;

pub use card::PresentationCardRenderer;
pub use chart::ChartRenderer;
pub use dashboard::DashboardRenderer;
pub use form::FormRenderer;
pub use image::ImageRenderer;
pub use list::ListRenderer;
pub use media::{AudioRenderer, VideoRenderer};
pub use message::MessageRenderer;
pub use table::TableRenderer;
pub use text::{CopyPasteTextRenderer, TextRenderer};
