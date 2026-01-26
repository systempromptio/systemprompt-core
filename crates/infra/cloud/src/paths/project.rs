use std::path::{Path, PathBuf};

use crate::constants::{dir_names, paths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectPath {
    Root,
    ProfilesDir,
    DockerDir,
    StorageDir,
    SessionsDir,
    Dockerfile,
    LocalCredentials,
    LocalTenants,
    LocalSession,
}

impl ProjectPath {
    #[must_use]
    pub const fn segments(&self) -> &'static [&'static str] {
        match self {
            Self::Root => &[paths::ROOT_DIR],
            Self::ProfilesDir => &[paths::ROOT_DIR, paths::PROFILES_DIR],
            Self::DockerDir => &[paths::ROOT_DIR, paths::DOCKER_DIR],
            Self::StorageDir => &[paths::ROOT_DIR, paths::STORAGE_DIR],
            Self::SessionsDir => &[paths::ROOT_DIR, dir_names::SESSIONS],
            Self::Dockerfile => &[paths::ROOT_DIR, paths::DOCKERFILE],
            Self::LocalCredentials => &[paths::ROOT_DIR, paths::CREDENTIALS_FILE],
            Self::LocalTenants => &[paths::ROOT_DIR, paths::TENANTS_FILE],
            Self::LocalSession => &[paths::ROOT_DIR, paths::SESSION_FILE],
        }
    }

    #[must_use]
    pub const fn is_dir(&self) -> bool {
        matches!(
            self,
            Self::Root | Self::ProfilesDir | Self::DockerDir | Self::StorageDir | Self::SessionsDir
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
    DockerDir,
    Dockerfile,
    Entrypoint,
    Dockerignore,
    Compose,
}

impl ProfilePath {
    #[must_use]
    pub const fn filename(&self) -> &'static str {
        match self {
            Self::Config => paths::PROFILE_CONFIG,
            Self::Secrets => paths::PROFILE_SECRETS,
            Self::DockerDir => paths::PROFILE_DOCKER_DIR,
            Self::Dockerfile => paths::DOCKERFILE,
            Self::Entrypoint => paths::ENTRYPOINT,
            Self::Dockerignore => paths::DOCKERIGNORE,
            Self::Compose => paths::COMPOSE_FILE,
        }
    }

    #[must_use]
    pub const fn is_docker_file(&self) -> bool {
        matches!(
            self,
            Self::Dockerfile | Self::Entrypoint | Self::Dockerignore | Self::Compose
        )
    }

    #[must_use]
    pub fn resolve(&self, profile_dir: &Path) -> PathBuf {
        match self {
            Self::Dockerfile | Self::Entrypoint | Self::Dockerignore | Self::Compose => profile_dir
                .join(paths::PROFILE_DOCKER_DIR)
                .join(self.filename()),
            _ => profile_dir.join(self.filename()),
        }
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
        let cwd = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(e) => {
                tracing::debug!(error = %e, "Failed to get current directory, using '.'");
                PathBuf::from(".")
            },
        };
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
    pub fn profile_docker_dir(&self, name: &str) -> PathBuf {
        self.profile_path(name, ProfilePath::DockerDir)
    }

    #[must_use]
    pub fn profile_dockerfile(&self, name: &str) -> PathBuf {
        self.profile_path(name, ProfilePath::Dockerfile)
    }

    #[must_use]
    pub fn profile_entrypoint(&self, name: &str) -> PathBuf {
        self.profile_path(name, ProfilePath::Entrypoint)
    }

    #[must_use]
    pub fn profile_dockerignore(&self, name: &str) -> PathBuf {
        self.profile_path(name, ProfilePath::Dockerignore)
    }

    #[must_use]
    pub fn profile_compose(&self, name: &str) -> PathBuf {
        self.profile_path(name, ProfilePath::Compose)
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
    pub fn sessions_dir(&self) -> PathBuf {
        self.resolve(ProjectPath::SessionsDir)
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
