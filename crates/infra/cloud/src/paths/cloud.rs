use std::path::{Path, PathBuf};

use crate::constants::{cli_session, credentials, dir_names, tenants};

use super::{resolve_path, ProjectContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudPath {
    Credentials,
    Tenants,
    CliSession,
    SessionsDir,
}

impl CloudPath {
    #[must_use]
    pub const fn default_filename(&self) -> &'static str {
        match self {
            Self::Credentials => credentials::DEFAULT_FILE_NAME,
            Self::Tenants => tenants::DEFAULT_FILE_NAME,
            Self::CliSession => cli_session::DEFAULT_FILE_NAME,
            Self::SessionsDir => dir_names::SESSIONS,
        }
    }

    #[must_use]
    pub const fn default_dirname(&self) -> &'static str {
        match self {
            Self::Credentials => credentials::DEFAULT_DIR_NAME,
            Self::Tenants => tenants::DEFAULT_DIR_NAME,
            Self::CliSession | Self::SessionsDir => cli_session::DEFAULT_DIR_NAME,
        }
    }

    #[must_use]
    pub const fn is_dir(&self) -> bool {
        matches!(self, Self::SessionsDir)
    }
}

#[derive(Debug, Clone)]
pub struct CloudPaths {
    base_dir: PathBuf,
    credentials_path: Option<PathBuf>,
    tenants_path: Option<PathBuf>,
}

impl CloudPaths {
    #[must_use]
    pub fn new(profile_dir: &Path) -> Self {
        Self {
            base_dir: profile_dir.join(credentials::DEFAULT_DIR_NAME),
            credentials_path: None,
            tenants_path: None,
        }
    }

    #[must_use]
    pub fn from_project_context(ctx: &ProjectContext) -> Self {
        Self {
            base_dir: ctx.systemprompt_dir(),
            credentials_path: Some(ctx.local_credentials()),
            tenants_path: Some(ctx.local_tenants()),
        }
    }

    #[must_use]
    pub fn from_config(
        profile_dir: &Path,
        credentials_path_str: &str,
        tenants_path_str: &str,
    ) -> Self {
        let credentials_path = if credentials_path_str.is_empty() {
            None
        } else {
            Some(resolve_path(profile_dir, credentials_path_str))
        };

        let tenants_path = if tenants_path_str.is_empty() {
            None
        } else {
            Some(resolve_path(profile_dir, tenants_path_str))
        };

        let base_dir = credentials_path
            .as_ref()
            .and_then(|p| p.parent())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                profile_dir
                    .ancestors()
                    .find(|p| p.file_name().is_some_and(|n| n == ".systemprompt"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| profile_dir.join(credentials::DEFAULT_DIR_NAME))
            });

        Self {
            base_dir,
            credentials_path,
            tenants_path,
        }
    }

    #[must_use]
    pub fn resolve(&self, path: CloudPath) -> PathBuf {
        match path {
            CloudPath::Credentials => self
                .credentials_path
                .clone()
                .unwrap_or_else(|| self.base_dir.join(credentials::DEFAULT_FILE_NAME)),
            CloudPath::Tenants => self
                .tenants_path
                .clone()
                .unwrap_or_else(|| self.base_dir.join(tenants::DEFAULT_FILE_NAME)),
            CloudPath::CliSession => self.base_dir.join(cli_session::DEFAULT_FILE_NAME),
            CloudPath::SessionsDir => self.base_dir.join(dir_names::SESSIONS),
        }
    }

    #[must_use]
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    #[must_use]
    pub fn exists(&self, path: CloudPath) -> bool {
        self.resolve(path).exists()
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn get_cloud_paths() -> anyhow::Result<CloudPaths> {
    let project_ctx = ProjectContext::discover();
    Ok(CloudPaths::from_project_context(&project_ctx))
}
