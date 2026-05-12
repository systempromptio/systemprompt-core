//! Schema installation from compile-time-registered
//! [`systemprompt_extension::Extension`] instances.

mod extension;

pub use extension::{
    install_extension_schemas, install_extension_schemas_full,
    install_extension_schemas_with_config,
};
