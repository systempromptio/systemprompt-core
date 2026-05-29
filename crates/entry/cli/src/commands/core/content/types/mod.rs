//! Serializable output shapes for the `core content` commands.
//!
//! Aggregates and re-exports the content, link, and pipeline output types so
//! command renderers share a single stable JSON surface.

mod content;
mod links;
mod pipeline;

pub use content::*;
pub use links::*;
pub use pipeline::*;
