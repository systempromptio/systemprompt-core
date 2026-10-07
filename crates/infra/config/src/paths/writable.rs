//! The directories a node writes at runtime, declared in one place.
//!
//! On a read-only root filesystem every path the server writes must be a
//! mounted volume. [`AppPaths::writable_roots`] is that contract: the boot
//! probe creates and write-tests each root, so a missing mount fails the
//! boot by name instead of failing the first request that writes. A
//! `gateway` node renders no `web/dist` and fetches no services bundle, so
//! it needs neither.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_manifest::profile::NodeRole;

use super::AppPaths;

/// A directory the server writes, with the name operators see in errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WritableRoot {
    pub name: &'static str,
    pub path: PathBuf,
}

impl WritableRoot {
    fn new(name: &'static str, path: impl Into<PathBuf>) -> Self {
        Self {
            name,
            path: path.into(),
        }
    }
}

impl AppPaths {
    pub fn writable_roots(&self, role: NodeRole, services_cache: &Path) -> Vec<WritableRoot> {
        let storage = self.storage();
        let mut roots = vec![
            WritableRoot::new("system.logs", self.system().logs()),
            WritableRoot::new("storage.files", storage.files()),
            WritableRoot::new("storage.exports", storage.exports()),
            WritableRoot::new("storage.data", storage.data()),
            WritableRoot::new("storage.scratch", storage.scratch()),
        ];
        if role.runs_workers() {
            roots.push(WritableRoot::new("web.dist", self.web().dist()));
            roots.push(WritableRoot::new("services.cache_dir", services_cache));
        }
        roots
    }
}
