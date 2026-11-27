//! The directories the shell hands in, and the names used inside a project.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

/// The directories the engine may use, filled by the shell. Nothing in this
/// crate derives them itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub home: PathBuf,
    pub data: PathBuf,
    pub config: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub tmp: PathBuf,
}

/// Names and limits used when looking for configuration: the project directory,
/// the settings file name, whether `.claude` and `.agents` directories are
/// honoured, and the per-file cap on instruction text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub project_dir: String,
    pub settings_file: String,
    pub compat_dirs: bool,
    pub instruction_limit: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            project_dir: ".harness".to_owned(),
            settings_file: "harness.json".to_owned(),
            compat_dirs: true,
            instruction_limit: 64 * 1024,
        }
    }
}

impl Layout {
    pub fn global_settings(&self, paths: &Paths) -> PathBuf {
        paths.config.join(&self.settings_file)
    }

    pub fn project_settings(&self, dir: &Path) -> [PathBuf; 2] {
        [
            dir.join(&self.settings_file),
            dir.join(&self.project_dir).join(&self.settings_file),
        ]
    }

    pub fn project_root(&self, dir: &Path) -> PathBuf {
        dir.join(&self.project_dir)
    }
}

pub(crate) fn expand_home(value: &str, home: &Path) -> PathBuf {
    if value == "~" {
        return home.to_path_buf();
    }
    value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
        .map_or_else(|| PathBuf::from(value), |rest| home.join(rest))
}

pub(crate) fn display_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
