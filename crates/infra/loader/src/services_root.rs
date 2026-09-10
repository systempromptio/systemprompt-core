//! Process-wide cell holding the services root the instance actually runs.
//!
//! With no bundle sources configured the root is the tree baked into the
//! image at `paths.services`. With sources it is a composed cache root, and
//! [`ServicesProvenance`] records how that root was chosen — including the
//! error that forced a fallback, so an instance running yesterday's bundle
//! can say so rather than looking healthy.
//!
//! The cell is installed once, before [`crate::ConfigLoader`] reads anything;
//! [`ServicesRootBootstrap::active_root_or`] is the accessor for callers that
//! must work whether or not boot has reached that point yet.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

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
    pub provenance: ServicesProvenance,
}

#[derive(Debug, Clone, Copy)]
pub struct ServicesRootBootstrap;

impl ServicesRootBootstrap {
    pub fn install(root: ActiveServicesRoot) -> &'static ActiveServicesRoot {
        drop(ACTIVE_ROOT.set(root));
        ACTIVE_ROOT.get().unwrap_or_else(|| unreachable_root())
    }

    #[must_use]
    pub fn get() -> Option<&'static ActiveServicesRoot> {
        ACTIVE_ROOT.get()
    }

    #[must_use]
    pub fn is_initialized() -> bool {
        ACTIVE_ROOT.get().is_some()
    }

    #[must_use]
    pub fn active_root_or(fallback: &str) -> PathBuf {
        ACTIVE_ROOT
            .get()
            .map_or_else(|| PathBuf::from(fallback), |root| root.path.clone())
    }

    #[must_use]
    pub fn active_path_or(fallback: &str, relative: &str) -> PathBuf {
        Self::active_root_or(fallback).join(relative)
    }
}

fn unreachable_root() -> &'static ActiveServicesRoot {
    static EMPTY: OnceLock<ActiveServicesRoot> = OnceLock::new();
    EMPTY.get_or_init(|| ActiveServicesRoot {
        path: Path::new(".").to_path_buf(),
        provenance: ServicesProvenance::Bundled,
    })
}
