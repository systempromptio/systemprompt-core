//! Filesystem path resolution for the cloud layer.
//!
//! Covers project discovery ([`DiscoveredProject`]), the typed project/profile
//! path enums ([`ProjectPath`], [`ProfilePath`], [`ProjectContext`]), the
//! container-side [`CloudPaths`], and the [`UnifiedContext`] that ties them
//! together for credential, tenant, and session lookups.

mod cloud;
mod context;
mod discovery;
mod project;

use std::path::{Path, PathBuf};

pub use cloud::{CloudPath, CloudPaths, get_cloud_paths};
pub use context::UnifiedContext;
pub use discovery::DiscoveredProject;
pub use project::{ProfilePath, ProjectContext, ProjectPath};
pub use systemprompt_models::profile::{expand_home, resolve_with_home};

#[must_use]
pub fn resolve_path(base_dir: &Path, path_str: &str) -> PathBuf {
    resolve_with_home(base_dir, path_str)
}
