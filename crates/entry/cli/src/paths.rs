use std::path::PathBuf;

use anyhow::{Context, Result};
use systemprompt_cloud::paths::{get_cloud_paths, CloudPath};
use systemprompt_cloud::ProjectContext;

#[derive(Debug)]
pub struct ResolvedPaths {
    project_ctx: ProjectContext,
    has_local_dir: bool,
}

impl ResolvedPaths {
    pub fn discover() -> Self {
        let project_ctx = ProjectContext::discover();
        let has_local_dir = project_ctx.systemprompt_dir().exists();
        Self {
            project_ctx,
            has_local_dir,
        }
    }

    pub fn sessions_dir(&self) -> Result<PathBuf> {
        if self.has_local_dir {
            Ok(self.project_ctx.sessions_dir())
        } else {
            let cloud_paths = get_cloud_paths()
                .context("Failed to resolve cloud paths from profile configuration")?;
            Ok(cloud_paths.resolve(CloudPath::SessionsDir))
        }
    }

    pub fn tenants_path(&self) -> Result<PathBuf> {
        if self.has_local_dir {
            Ok(self.project_ctx.local_tenants())
        } else {
            let cloud_paths = get_cloud_paths().context("Failed to resolve cloud paths")?;
            Ok(cloud_paths.resolve(CloudPath::Tenants))
        }
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.project_ctx.profiles_dir()
    }
}
