//! Bootstrap sequence orchestration.
//!
//! This module provides type-safe bootstrap sequencing that enforces
//! initialization dependencies at compile time. The sequence ensures
//! that secrets cannot be initialized without a profile, and paths
//! cannot be initialized without secrets.

use std::marker::PhantomData;
use std::path::Path;

use anyhow::{Context, Result};

use crate::profile_bootstrap::ProfileBootstrap;
use crate::secrets_bootstrap::SecretsBootstrap;
use crate::{AppPaths, Config, PathsConfig};

pub trait BootstrapState {}

#[derive(Debug, Clone, Copy)]
pub struct Uninitialized;
impl BootstrapState for Uninitialized {}

#[derive(Debug, Clone, Copy)]
pub struct ProfileInitialized;
impl BootstrapState for ProfileInitialized {}

#[derive(Debug, Clone, Copy)]
pub struct SecretsInitialized;
impl BootstrapState for SecretsInitialized {}

#[derive(Debug, Clone, Copy)]
pub struct PathsInitialized;
impl BootstrapState for PathsInitialized {}

#[derive(Debug)]
pub struct BootstrapSequence<S: BootstrapState> {
    _state: PhantomData<S>,
}

impl Default for BootstrapSequence<Uninitialized> {
    fn default() -> Self {
        Self::new()
    }
}

impl BootstrapSequence<Uninitialized> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _state: PhantomData,
        }
    }

    pub fn with_profile(self, path: &Path) -> Result<BootstrapSequence<ProfileInitialized>> {
        let Self { _state: _ } = self;
        ProfileBootstrap::init_from_path(path)
            .with_context(|| format!("Profile initialization failed from: {}", path.display()))?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    #[must_use]
    pub const fn skip_profile(self) -> Self {
        self
    }
}

impl BootstrapSequence<ProfileInitialized> {
    pub fn with_secrets(self) -> Result<BootstrapSequence<SecretsInitialized>> {
        let Self { _state: _ } = self;
        SecretsBootstrap::init().context("Secrets initialization failed")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    #[must_use]
    pub const fn skip_secrets(self) -> Self {
        self
    }
}

impl BootstrapSequence<SecretsInitialized> {
    pub fn with_paths(self) -> Result<BootstrapSequence<PathsInitialized>> {
        let Self { _state: _ } = self;
        let profile = ProfileBootstrap::get()?;
        AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
        Config::try_init().context("Failed to initialize configuration")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    pub fn with_paths_config(
        self,
        paths_config: &PathsConfig,
    ) -> Result<BootstrapSequence<PathsInitialized>> {
        let Self { _state: _ } = self;
        AppPaths::init(paths_config).context("Failed to initialize paths")?;
        Config::try_init().context("Failed to initialize configuration")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    #[must_use]
    pub const fn skip_paths(self) -> Self {
        self
    }
}

impl BootstrapSequence<PathsInitialized> {
    #[must_use]
    pub const fn complete(self) -> BootstrapComplete {
        let Self { _state: _ } = self;
        BootstrapComplete { _private: () }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BootstrapComplete {
    _private: (),
}

pub mod presets {
    use std::path::Path;

    use anyhow::Result;

    use super::{
        BootstrapComplete, BootstrapSequence, ProfileInitialized, SecretsInitialized, Uninitialized,
    };

    pub fn full(profile_path: &Path) -> Result<BootstrapComplete> {
        Ok(BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()?
            .with_paths()?
            .complete())
    }

    pub fn profile_and_secrets(
        profile_path: &Path,
    ) -> Result<BootstrapSequence<SecretsInitialized>> {
        BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()
    }

    pub fn profile_only(profile_path: &Path) -> Result<BootstrapSequence<ProfileInitialized>> {
        BootstrapSequence::<Uninitialized>::new().with_profile(profile_path)
    }
}
