//! Process-wide cell holding the services root the instance actually runs.
//!
//! With no bundle sources configured the root is the tree baked into the
//! image at `paths.services`. With sources it is a composed cache root, and
//! [`ServicesProvenance`] records how that root was chosen — including the
//! error that forced a fallback, so an instance running yesterday's bundle
//! can say so rather than looking healthy.
//!
//! The cell is installed once, before [`crate::ConfigLoader`] reads anything;
//! [`ServicesRootBootstrap::require`] is the accessor for callers that need
//! it, and fails when boot has not installed it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::error::ConfigLoadError;

static ACTIVE_ROOT: OnceLock<ActiveServicesRoot> = OnceLock::new();

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

#[derive(Debug, Clone, Copy)]
pub struct ServicesRootBootstrap;

impl ServicesRootBootstrap {
    pub fn install(root: ActiveServicesRoot) -> &'static ActiveServicesRoot {
        ACTIVE_ROOT.get_or_init(|| root)
    }

    #[must_use]
    pub fn get() -> Option<&'static ActiveServicesRoot> {
        ACTIVE_ROOT.get()
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        ACTIVE_ROOT.get().is_some()
    }

    pub fn require() -> Result<&'static ActiveServicesRoot, ConfigLoadError> {
        ACTIVE_ROOT
            .get()
            .ok_or(ConfigLoadError::ServicesRootUninitialised)
    }
}
