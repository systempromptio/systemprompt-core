//! Canonical configured and managed inventory with explicit adoption and
//! observed membership.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod baseline;
pub(crate) mod catalog;
mod configured_files;
mod projection;
mod publish_latest;
mod publish_provenance;
mod repository;
mod service;
mod types;
pub use catalog::{configured_inventory_fingerprint, scan_configured_inventory};
pub use publish_latest::{LatestPublication, LatestPublicationStatus, PublishGuard};
pub(crate) use repository::IncomingRevision;
pub use service::InventoryService;
pub use types::*;

mod installation_coverage;
pub use installation_coverage::InstallationCoverage;
