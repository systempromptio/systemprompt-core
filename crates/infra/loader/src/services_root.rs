//! The services root an instance actually runs.
//!
//! With no bundle sources configured the root is the tree baked into the
//! image at `paths.services`. With sources it is a composed cache root, and
//! [`ServicesProvenance`] records how that root was chosen — including the
//! error that forced a fallback, so an instance running yesterday's bundle
//! can say so rather than looking healthy.
//!
//! The boot context installs the root once, before [`crate::ConfigLoader`]
//! reads anything; [`crate::BootContext::require_services_root`] answers the
//! profile's baked tree while no bundle bootstrap has installed one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use systemprompt_manifest::profile::Profile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServicesProvenance {
    Bundled,

    Fetched {
        composed_hash: String,
        versions: BTreeMap<String, String>,
    },

    LastGood {
        composed_hash: String,
        error: String,
    },

    BundledFallback {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveServicesRoot {
    pub path: PathBuf,
    pub base: PathBuf,
    pub provenance: ServicesProvenance,
}

impl ActiveServicesRoot {
    #[must_use]
    pub fn baked(profile: &Profile) -> Self {
        let path = PathBuf::from(&profile.paths.services);
        Self {
            base: path.clone(),
            path,
            provenance: ServicesProvenance::Bundled,
        }
    }
}
