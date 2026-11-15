//! The secrets sources a composition contributes.
//!
//! A profile with `secrets.source: external` names a `kind`; the provider for
//! that kind is declared on the composition and handed to the secrets
//! bootstrap as [`SecretsSources`]. Core ships no external kind of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_manifest::profile::ExternalSecretsConfig;

use super::SecretsBootstrapError;
use super::provider::DynSecretsProvider;

pub type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub type SharedSecretsProvider = Box<dyn DynSecretsProvider>;

pub type SecretsSourceFactory =
    fn(&ExternalSecretsConfig, EnvLookup) -> Result<SharedSecretsProvider, SecretsBootstrapError>;

#[derive(Debug, Clone, Copy)]
pub struct SecretsSourceRegistration {
    pub kind: &'static str,
    pub factory: SecretsSourceFactory,
}

/// The external secrets sources the bootstrap may build, by kind.
#[derive(Debug, Clone, Default)]
pub struct SecretsSources(Arc<[SecretsSourceRegistration]>);

impl SecretsSources {
    #[must_use]
    pub fn new(registrations: Vec<SecretsSourceRegistration>) -> Self {
        Self(registrations.into())
    }

    #[must_use]
    pub fn find(&self, kind: &str) -> Option<&SecretsSourceRegistration> {
        self.0.iter().find(|registration| registration.kind == kind)
    }

    #[must_use]
    pub fn kinds(&self) -> Vec<&'static str> {
        let mut kinds: Vec<&'static str> =
            self.0.iter().map(|registration| registration.kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }
}

pub fn process_env_lookup() -> EnvLookup {
    Box::new(systemprompt_manifest::read_env_optional)
}

pub fn build_external_provider(
    cfg: &ExternalSecretsConfig,
    env: EnvLookup,
    sources: &SecretsSources,
) -> Result<SharedSecretsProvider, SecretsBootstrapError> {
    let registration =
        sources
            .find(&cfg.kind)
            .ok_or_else(|| SecretsBootstrapError::UnknownExternalKind {
                kind: cfg.kind.clone(),
                known: sources.kinds().into_iter().map(str::to_owned).collect(),
            })?;
    (registration.factory)(cfg, env)
}
