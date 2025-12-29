use std::path::{Path, PathBuf};

use crate::constants::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectPath {
    Root,
    ProfilesDir,
    DockerDir,
    StorageDir,
    Dockerfile,
    LocalCredentials,
    LocalTenants,
}

impl ProjectPath {
    #[must_use]
    pub const fn segments(&self) -> &'static [&'static str] {
        match self {
            Self::Root => &[paths::ROOT_DIR],
            Self::ProfilesDir => &[paths::ROOT_DIR, paths::PROFILES_DIR],
            Self::DockerDir => &[paths::ROOT_DIR, paths::DOCKER_DIR],
            Self::StorageDir => &[paths::ROOT_DIR, paths::STORAGE_DIR],
            Self::Dockerfile => &[paths::ROOT_DIR, paths::DOCKERFILE],
            Self::LocalCredentials => &[paths::ROOT_DIR, paths::CREDENTIALS_FILE],
            Self::LocalTenants => &[paths::ROOT_DIR, paths::TENANTS_FILE],
        }
    }

    #[must_use]
    pub const fn is_dir(&self) -> bool {
        matches!(
            self,
            Self::Root | Self::ProfilesDir | Self::DockerDir | Self::StorageDir
        )
    }

    #[must_use]
    pub fn resolve(&self, project_root: &Path) -> PathBuf {
        let mut path = project_root.to_path_buf();
        for segment in self.segments() {
            path.push(segment);
        }
        path
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfilePath {
    Config,
    Secrets,
}

impl ProfilePath {
    #[must_use]
    pub const fn filename(&self) -> &'static str {
        match self {
            Self::Config => paths::PROFILE_CONFIG,
            Self::Secrets => paths::PROFILE_SECRETS,
        }
    }

    #[must_use]
    pub fn resolve(&self, profile_dir: &Path) -> PathBuf {
        profile_dir.join(self.filename())
    }
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    root: PathBuf,
}

impl ProjectContext {
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    #[must_use]
    pub fn discover() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::discover_from(&cwd)
    }

    #[must_use]
    pub fn discover_from(start: &Path) -> Self {
        let mut current = start.to_path_buf();
        loop {
            if current.join(paths::ROOT_DIR).is_dir() {
                return Self::new(current);
            }
            if !current.pop() {
                break;
            }
        }
        Self::new(start.to_path_buf())
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn resolve(&self, path: ProjectPath) -> PathBuf {
        path.resolve(&self.root)
    }

    #[must_use]
    pub fn systemprompt_dir(&self) -> PathBuf {
        self.resolve(ProjectPath::Root)
    }

    #[must_use]
    pub fn profiles_dir(&self) -> PathBuf {
        self.resolve(ProjectPath::ProfilesDir)
    }

    #[must_use]
    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(name)
    }

    #[must_use]
    pub fn profile_path(&self, name: &str, path: ProfilePath) -> PathBuf {
        path.resolve(&self.profile_dir(name))
    }

    #[must_use]
    pub fn docker_dir(&self) -> PathBuf {
        self.resolve(ProjectPath::DockerDir)
    }

    #[must_use]
    pub fn storage_dir(&self) -> PathBuf {
        self.resolve(ProjectPath::StorageDir)
    }

    #[must_use]
    pub fn dockerfile(&self) -> PathBuf {
        self.resolve(ProjectPath::Dockerfile)
    }

    #[must_use]
    pub fn local_credentials(&self) -> PathBuf {
        self.resolve(ProjectPath::LocalCredentials)
    }

    #[must_use]
    pub fn local_tenants(&self) -> PathBuf {
        self.resolve(ProjectPath::LocalTenants)
    }

    #[must_use]
    pub fn exists(&self, path: ProjectPath) -> bool {
        self.resolve(path).exists()
    }

    #[must_use]
    pub fn profile_exists(&self, name: &str) -> bool {
        self.profile_dir(name).exists()
    }
}
