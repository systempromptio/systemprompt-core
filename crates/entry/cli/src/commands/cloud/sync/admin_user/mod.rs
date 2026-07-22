//! Synchronise the authenticated cloud user as an admin across local profiles.
//!
//! Discovers profile databases, then creates or promotes the cloud user to
//! admin in each. Public surface: [`CloudUser`], [`SyncResult`], the discovery
//! helpers, and the per-database / all-profiles sync entry points.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod discovery;
mod sync;
mod types;

pub use discovery::{discover_profiles, print_discovery_summary, process_profile_entry};
pub use sync::{print_sync_results, sync_admin_to_all_profiles, sync_admin_to_database};
pub use types::{CloudUser, ProfileDiscoveryResult, ProfileEntryResult, ProfileInfo, ProfileSkipReason, SyncResult};
