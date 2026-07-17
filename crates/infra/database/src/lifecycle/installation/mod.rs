//! Schema installation from compile-time-registered
//! [`systemprompt_extension::Extension`] instances.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod extension;
mod prepare;
mod seeds;

pub use extension::{
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config,
};
