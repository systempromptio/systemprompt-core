//! On-disk catalogue loaders for the bridge manifest.
//!
//! Each loader scans the services tree (or projects the `ServicesConfig`)
//! into the signed `*Entry` records the manifest carries. Loaders take a
//! services-root [`std::path::Path`], the `ServicesConfig`, and the API
//! external URL where endpoints must be resolved — never an `AppContext` or a
//! database handle. Disk and parse failures surface as
//! [`MarketplaceError::Catalog`](crate::error::MarketplaceError).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod agents;
mod artifacts;
mod content;
mod fingerprint;
mod hooks;
mod mcp;
mod plugins;
mod skills;

pub use agents::load_agents;
pub use artifacts::load_artifacts;
pub use content::CatalogContent;
pub use hooks::load_hooks;
pub use mcp::{disabled_mcp_server_names, load_managed_mcp_servers};
pub use plugins::{
    artifact_owners, load_plugins, plugin_bundles, plugin_bundles_cached, selects_artifact,
};
pub use skills::load_skills;
