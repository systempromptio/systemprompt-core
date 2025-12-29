mod cloud;
mod project;

use std::path::{Path, PathBuf};

pub use cloud::{get_cloud_paths, CloudPath, CloudPaths};
pub use project::{ProfilePath, ProjectContext, ProjectPath};
pub use systemprompt_models::profile::{expand_home, resolve_with_home};

#[must_use]
pub fn resolve_path(base_dir: &Path, path_str: &str) -> PathBuf {
    resolve_with_home(base_dir, path_str)
}
