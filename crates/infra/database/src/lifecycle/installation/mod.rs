//! Schema and seed installation pipelines.
//!
//! Split into two cohesive halves:
//! - `module` — installation from on-disk
//!   [`systemprompt_models::modules::Module`] descriptors used by the legacy
//!   loader path.
//! - `extension` — installation from compile-time-registered
//!   [`systemprompt_extension::Extension`] instances (the modern path).

mod extension;
mod module;
mod util;

pub use extension::{install_extension_schemas, install_extension_schemas_with_config};
pub use module::{
    ModuleInstaller, install_module_schemas_from_source, install_module_seeds_from_path,
    install_schema, install_seed,
};
