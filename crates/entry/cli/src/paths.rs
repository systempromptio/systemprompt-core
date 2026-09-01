//! Resolved filesystem paths the CLI operates against.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_cloud::ProjectContext;
use systemprompt_cloud::paths::{CloudPath, get_cloud_paths};

#[derive(Debug)]
pub struct ResolvedPaths {
    project_ctx: ProjectContext,
    has_local_dir: bool,
}

impl ResolvedPaths {
    // Why: preferred once a profile is resolved, so the tenant and session stores
    // depend on the profile rather than on the directory the process started
    // in.
    pub fn for_root(root: &Path) -> Self {
        let project_ctx = ProjectContext::discover_from(root);
        let has_local_dir = project_ctx.systemprompt_dir().exists();
        Self {
            project_ctx,
            has_local_dir,
        }
    }

    pub fn from_profile(profile: &systemprompt_models::Profile) -> Self {
        Self::for_root(Path::new(&profile.paths.system))
    }

    // Why: only correct before a profile is resolved; afterwards use
    // `from_profile`, or the same command answers differently depending on the
    // caller's cwd.
    pub fn discover() -> Self {
        let project_ctx = ProjectContext::discover();
        let has_local_dir = project_ctx.systemprompt_dir().exists();
        tracing::debug!(
            root = %project_ctx.root().display(),
            has_local_dir,
            "Resolved project root by walking up from the current directory"
        );
        Self {
            project_ctx,
            has_local_dir,
        }
    }

    pub fn sessions_dir(&self) -> PathBuf {
        if self.has_local_dir {
            self.project_ctx.sessions_dir()
        } else {
            let cloud_paths = get_cloud_paths();
            cloud_paths.resolve(CloudPath::SessionsDir)
        }
    }

    pub fn tenants_path(&self) -> PathBuf {
        if self.has_local_dir {
            self.project_ctx.local_tenants()
        } else {
            let cloud_paths = get_cloud_paths();
            cloud_paths.resolve(CloudPath::Tenants)
        }
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.project_ctx.profiles_dir()
    }
}
