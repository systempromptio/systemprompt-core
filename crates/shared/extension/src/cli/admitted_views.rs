//! The open admitted views a command reads about a target profile's
//! composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::{Admitted, AdmittedAssets, AdmittedConfigs, AdmittedMigrations};

/// The declared migrations, configuration sections and assets of an admitted
/// composition, each open: the same views the kernel seeds for a member's
/// command, and what [`CommandHost::admitted_views`](crate::cli::CommandHost::admitted_views)
/// returns for a profile at a path.
#[derive(Debug, Clone)]
pub struct AdmittedViews {
    pub migrations: AdmittedMigrations,
    pub configs: AdmittedConfigs,
    pub assets: AdmittedAssets,
}

impl AdmittedViews {
    #[must_use]
    pub fn of(admitted: &Admitted) -> Self {
        Self {
            migrations: AdmittedMigrations::of(admitted.built()),
            configs: AdmittedConfigs::of(admitted.built()),
            assets: AdmittedAssets::of(admitted.built()),
        }
    }
}
