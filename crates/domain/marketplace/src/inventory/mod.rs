//! Canonical configured and managed inventory with explicit adoption and
//! observed membership.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod baseline;
mod captures;
mod catalog;
mod membership;
mod projection;
mod publish_latest;
mod publish_provenance;
mod repository;
mod service;
mod sources;
mod types;
pub(crate) use captures::IncomingRevision;
pub use catalog::scan_configured_inventory;
pub use publish_latest::{LatestPublication, LatestPublicationStatus, PublishGuard};
pub use service::InventoryService;
pub use types::*;

mod installation_coverage;
pub use installation_coverage::{InstallationCoverage, InstallationCoverageStatus};
