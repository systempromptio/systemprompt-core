//! Top-level [`DataImpact`] classification of the command tree.
//!
//! Each group that can open the profile's database classifies its own
//! variants exhaustively next to its definition. The groups classified whole
//! here never open that database: `cloud` targets tenants through the cloud
//! API, `analytics` is read-only, and `web`/`build` edit files on disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Commands;
use crate::descriptor::DataImpact;

impl Commands {
    pub const fn data_impact(&self) -> DataImpact {
        match self {
            Self::Core(cmd) => cmd.data_impact(),
            Self::Infra(cmd) => cmd.data_impact(),
            Self::Admin(cmd) => cmd.data_impact(),
            Self::Plugins(cmd) => cmd.data_impact(),
            Self::Cloud(_) | Self::Analytics(_) | Self::Web(_) | Self::Build(_) => {
                DataImpact::Preserving
            },
        }
    }
}
