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
use crate::secrets::SecretsBootstrap;
use crate::{AppPaths, Config, PathsConfig};

/// Marker trait for bootstrap states.
pub trait BootstrapState {}

/// Initial state - nothing initialized.
#[derive(Debug, Clone, Copy)]
pub struct Uninitialized;
impl BootstrapState for Uninitialized {}

/// Profile has been initialized.
#[derive(Debug, Clone, Copy)]
pub struct ProfileInitialized;
impl BootstrapState for ProfileInitialized {}

/// Secrets have been initialized (requires profile).
#[derive(Debug, Clone, Copy)]
pub struct SecretsInitialized;
impl BootstrapState for SecretsInitialized {}

/// Paths have been initialized (requires secrets).
#[derive(Debug, Clone, Copy)]
pub struct PathsInitialized;
impl BootstrapState for PathsInitialized {}

/// Type-safe bootstrap sequence builder.
///
/// Uses the type state pattern to ensure initialization happens in the
/// correct order: Profile -> Secrets -> Paths
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
    /// Creates a new bootstrap sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _state: PhantomData,
        }
    }

    /// Initializes the profile from a path.
    ///
    /// This must be called first before secrets or paths can be initialized.
    #[allow(clippy::unused_self)]
    pub fn with_profile(self, path: &Path) -> Result<BootstrapSequence<ProfileInitialized>> {
        ProfileBootstrap::init_from_path(path).with_context(|| {
            format!("Profile initialization failed from: {}", path.display())
        })?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Skips profile initialization (for commands that don't need it).
    #[must_use]
    pub const fn skip_profile(self) -> Self {
        self
    }
}

impl BootstrapSequence<ProfileInitialized> {
    /// Initializes secrets from the loaded profile.
    ///
    /// Requires profile to be initialized first.
    #[allow(clippy::unused_self)]
    pub fn with_secrets(self) -> Result<BootstrapSequence<SecretsInitialized>> {
        SecretsBootstrap::init().context("Secrets initialization failed")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Skips secrets initialization but allows moving forward.
    ///
    /// Useful for commands that need profile but not secrets.
    #[must_use]
    pub const fn skip_secrets(self) -> Self {
        self
    }
}

impl BootstrapSequence<SecretsInitialized> {
    /// Initializes application paths from the profile configuration.
    ///
    /// Requires secrets to be initialized first.
    #[allow(clippy::unused_self)]
    pub fn with_paths(self) -> Result<BootstrapSequence<PathsInitialized>> {
        let profile = ProfileBootstrap::get()?;
        AppPaths::init(&profile.paths).context("Failed to initialize paths")?;
        Config::try_init().context("Failed to initialize configuration")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Initializes paths with custom configuration.
    #[allow(clippy::unused_self)]
    pub fn with_paths_config(
        self,
        paths_config: &PathsConfig,
    ) -> Result<BootstrapSequence<PathsInitialized>> {
        AppPaths::init(paths_config).context("Failed to initialize paths")?;
        Config::try_init().context("Failed to initialize configuration")?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Skips paths initialization.
    #[must_use]
    pub const fn skip_paths(self) -> Self {
        self
    }
}

impl BootstrapSequence<PathsInitialized> {
    /// Returns a reference to indicate bootstrap is complete.
    #[must_use]
    #[allow(clippy::unused_self)]
    pub const fn complete(&self) -> BootstrapComplete {
        BootstrapComplete { _private: () }
    }
}

/// Proof that bootstrap completed successfully.
#[derive(Debug, Clone, Copy)]
pub struct BootstrapComplete {
    _private: (),
}

/// Convenience functions for common bootstrap patterns.
pub mod presets {
    use std::path::Path;

    use anyhow::Result;

    use super::{
        BootstrapComplete, BootstrapSequence, ProfileInitialized, SecretsInitialized, Uninitialized,
    };

    /// Full bootstrap: profile + secrets + paths.
    pub fn full(profile_path: &Path) -> Result<BootstrapComplete> {
        Ok(BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()?
            .with_paths()?
            .complete())
    }

    /// Profile and secrets only (no paths).
    pub fn profile_and_secrets(
        profile_path: &Path,
    ) -> Result<BootstrapSequence<SecretsInitialized>> {
        BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()
    }

    /// Profile only.
    pub fn profile_only(profile_path: &Path) -> Result<BootstrapSequence<ProfileInitialized>> {
        BootstrapSequence::<Uninitialized>::new().with_profile(profile_path)
    }
}
