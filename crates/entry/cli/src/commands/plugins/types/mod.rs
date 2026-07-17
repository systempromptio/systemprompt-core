//! Serialisable output types for the `plugins` command group.
//!
//! Re-exports the extension-detail, capability-listing, and validation result
//! shapes from their respective submodules so subcommands can name them from a
//! single `super::types` path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod capability_types;
mod extension_types;
mod validation_types;

pub use capability_types::*;
pub use extension_types::*;
pub use validation_types::*;
