use std::path::PathBuf;

use systemprompt_cloud::ProjectContext;
use systemprompt_cloud::paths::{CloudPath, get_cloud_paths};

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
