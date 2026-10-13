//! Registry of extension-supplied secrets sources.
//!
//! A profile with `secrets.source: external` names a `kind`; the provider for
//! that kind is contributed at compile time with
//! [`register_secrets_source!`](crate::register_secrets_source) and collected
//! through `inventory`, the same way authorization subject providers are.
//! Core ships no external kind of its own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_manifest::profile::ExternalSecretsConfig;

use super::SecretsBootstrapError;
use super::provider::DynSecretsProvider;

pub type EnvLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

pub type SharedSecretsProvider = Box<dyn DynSecretsProvider>;

pub type SecretsSourceFactory =
    fn(&ExternalSecretsConfig, EnvLookup) -> Result<SharedSecretsProvider, SecretsBootstrapError>;

pub struct SecretsSourceRegistration {
    pub kind: &'static str,
    pub factory: SecretsSourceFactory,
}

inventory::collect!(SecretsSourceRegistration);

/// The process environment as an [`EnvLookup`] — what the boot loader hands
/// every registered factory.
#[must_use]
pub fn process_env_lookup() -> EnvLookup {
    Box::new(|name| std::env::var(name).ok())
}

pub fn build_external_provider(
    cfg: &ExternalSecretsConfig,
    env: EnvLookup,
) -> Result<SharedSecretsProvider, SecretsBootstrapError> {
    let registration = inventory::iter::<SecretsSourceRegistration>()
        .find(|registration| registration.kind == cfg.kind)
        .ok_or_else(|| SecretsBootstrapError::UnknownExternalKind {
            kind: cfg.kind.clone(),
            known: registered_external_kinds()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        })?;
    (registration.factory)(cfg, env)
}

#[must_use]
pub fn registered_external_kinds() -> Vec<&'static str> {
    let mut kinds: Vec<&'static str> = inventory::iter::<SecretsSourceRegistration>()
        .map(|registration| registration.kind)
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

#[macro_export]
macro_rules! register_secrets_source {
    ($kind:expr, $factory:expr $(,)?) => {
        $crate::__inventory::submit! {
            $crate::SecretsSourceRegistration {
                kind: $kind,
                factory: $factory,
            }
        }
    };
}
