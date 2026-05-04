//! Bootstrap sequence orchestration.
//!
//! This module provides type-safe bootstrap sequencing that enforces
//! initialization dependencies at compile time. The sequence ensures
//! that secrets cannot be initialized without a profile.

use std::marker::PhantomData;
use std::path::Path;

use crate::error::ConfigResult;

mod manifest;
mod profile;
mod secrets;

pub use manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
pub use profile::{ProfileBootstrap, ProfileBootstrapError};
pub use secrets::{
    JWT_SECRET_MIN_LENGTH, SecretsBootstrap, SecretsBootstrapError, build_loaded_secrets_message,
    load_secrets_from_path, log_secrets_issue, log_secrets_skip, log_secrets_warn,
};

/// Marker trait implemented by every bootstrap state.
pub trait BootstrapState {}

/// Initial state — neither profile nor secrets installed.
#[derive(Debug, Clone, Copy)]
pub struct Uninitialized;
impl BootstrapState for Uninitialized {}

/// State after a profile has been installed.
#[derive(Debug, Clone, Copy)]
pub struct ProfileInitialized;
impl BootstrapState for ProfileInitialized {}

/// State after both profile and secrets have been installed.
#[derive(Debug, Clone, Copy)]
pub struct SecretsInitialized;
impl BootstrapState for SecretsInitialized {}

/// Type-state container that enforces the bootstrap sequence at
/// compile time.
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
    /// Create a new uninitialized sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _state: PhantomData,
        }
    }

    /// Install the profile at `path` and transition to
    /// [`ProfileInitialized`].
    ///
    /// # Errors
    ///
    /// Returns the same variants as
    /// [`ProfileBootstrap::init_from_path`].
    pub fn with_profile(self, path: &Path) -> ConfigResult<BootstrapSequence<ProfileInitialized>> {
        let Self { _state: _ } = self;
        ProfileBootstrap::init_from_path(path)?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Skip profile installation (caller is responsible for ensuring
    /// the profile is already installed).
    #[must_use]
    pub const fn skip_profile(self) -> Self {
        self
    }
}

impl BootstrapSequence<ProfileInitialized> {
    /// Install secrets and transition to [`SecretsInitialized`].
    ///
    /// # Errors
    ///
    /// Returns the same variants as [`SecretsBootstrap::init`].
    pub fn with_secrets(self) -> ConfigResult<BootstrapSequence<SecretsInitialized>> {
        let Self { _state: _ } = self;
        SecretsBootstrap::init()?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }

    /// Skip secrets installation.
    #[must_use]
    pub const fn skip_secrets(self) -> Self {
        self
    }
}

impl BootstrapSequence<SecretsInitialized> {
    /// Finalize the sequence into a [`BootstrapComplete`] token.
    #[must_use]
    pub const fn complete(self) -> BootstrapComplete {
        let Self { _state: _ } = self;
        BootstrapComplete { _private: () }
    }
}

/// Witness type returned by [`BootstrapSequence::complete`] proving
/// that profile and secrets have both been installed.
#[derive(Debug, Clone, Copy)]
pub struct BootstrapComplete {
    _private: (),
}

/// Common bootstrap presets used by the CLI/API entry crates.
pub mod presets {
    use std::path::Path;

    use super::{BootstrapSequence, ProfileInitialized, SecretsInitialized, Uninitialized};
    use crate::error::ConfigResult;

    /// Install profile and secrets in one shot.
    ///
    /// # Errors
    ///
    /// Returns the union of [`BootstrapSequence::with_profile`] and
    /// [`BootstrapSequence::with_secrets`] errors.
    pub fn profile_and_secrets(
        profile_path: &Path,
    ) -> ConfigResult<BootstrapSequence<SecretsInitialized>> {
        BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()
    }

    /// Install only the profile.
    ///
    /// # Errors
    ///
    /// Returns the same variants as [`BootstrapSequence::with_profile`].
    pub fn profile_only(
        profile_path: &Path,
    ) -> ConfigResult<BootstrapSequence<ProfileInitialized>> {
        BootstrapSequence::<Uninitialized>::new().with_profile(profile_path)
    }
}
