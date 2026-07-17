//! Serializable output shapes for the `core content` commands.
//!
//! Aggregates and re-exports the content, link, and pipeline output types so
//! command renderers share a single stable JSON surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod links;
mod pipeline;

pub use content::*;
pub use links::*;
pub use pipeline::*;
