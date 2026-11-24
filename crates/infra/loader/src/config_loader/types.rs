//! Config-loader input/output types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_manifest::services::ServicesConfig;

use super::parse::Interpolator;

pub(super) struct IncludeResolveCtx<'a> {
    pub visited: &'a mut HashSet<PathBuf>,
    pub merged: &'a mut ServicesConfig,
    pub chain: Vec<PathBuf>,
    pub interpolator: Interpolator,
    pub staged: Option<&'a StagedSubstitute>,
}

#[derive(Debug, Clone)]
pub(super) struct StagedSubstitute {
    live: PathBuf,
    staged: PathBuf,
}

impl StagedSubstitute {
    pub(super) fn new(live: &Path, staged: &Path) -> Self {
        Self {
            live: anchored(live),
            staged: staged.to_path_buf(),
        }
    }

    pub(super) fn source_for(&self, include: &Path) -> Option<&Path> {
        (anchored(include) == self.live).then_some(self.staged.as_path())
    }
}

fn anchored(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    match (path.parent().map(fs::canonicalize), path.file_name()) {
        (Some(Ok(dir)), Some(name)) => dir.join(name),
        _ => path.to_path_buf(),
    }
}
