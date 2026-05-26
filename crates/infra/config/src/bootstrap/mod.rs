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
    SecretsBootstrap, SecretsBootstrapError, build_loaded_secrets_message, load_secrets_from_path,
    log_secrets_issue, log_secrets_skip, log_secrets_warn,
};

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

    pub fn with_profile(self, path: &Path) -> ConfigResult<BootstrapSequence<ProfileInitialized>> {
        let Self { _state: _ } = self;
        ProfileBootstrap::init_from_path(path)?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }
}

impl BootstrapSequence<ProfileInitialized> {
    pub fn with_secrets(self) -> ConfigResult<BootstrapSequence<SecretsInitialized>> {
        let Self { _state: _ } = self;
        SecretsBootstrap::init()?;

        Ok(BootstrapSequence {
            _state: PhantomData,
        })
    }
}

impl BootstrapSequence<SecretsInitialized> {
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

    use super::{BootstrapSequence, ProfileInitialized, SecretsInitialized, Uninitialized};
    use crate::error::ConfigResult;

    pub fn profile_and_secrets(
        profile_path: &Path,
    ) -> ConfigResult<BootstrapSequence<SecretsInitialized>> {
        BootstrapSequence::<Uninitialized>::new()
            .with_profile(profile_path)?
            .with_secrets()
    }

    pub fn profile_only(
        profile_path: &Path,
    ) -> ConfigResult<BootstrapSequence<ProfileInitialized>> {
        BootstrapSequence::<Uninitialized>::new().with_profile(profile_path)
    }
}
