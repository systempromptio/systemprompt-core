//! Bootstrap entry points.
//!
//! [`LoadedProfile`] and [`SecretsService`] are the owned profile and secrets
//! the boot sequence builds, in that order. The ordering is a runtime
//! invariant of the entry-crate boot sequence — which interleaves credential
//! and routing resolution between the two steps — rather than a type-state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod key_material;
mod manifest;
mod master_key;
mod profile;
mod secrets;

pub use key_material::KeyMaterialError;
pub use manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
pub use master_key::{ENCRYPTION_MASTER_KEY_BYTES, decode_master_key, generate_master_key};
pub use profile::{LoadedProfile, ProfileBootstrapError};
pub use secrets::{
    BOOT_REQUIRED_SECRET_KEYS, DynSecretsProvider, EnvLookup, PINNED_SECRET_KEYS, ReloadOutcome,
    ResolvedSource, SIGNING_KEY_PEM, SecretsBootstrapError, SecretsService, SecretsDocument,
    SecretsFetch, SecretsProvider, SecretsSourceFactory, SecretsSourceRegistration, SecretsStatus,
    SharedSecretsProvider, VaultAttemptFailure, VaultError, VaultKvProvider,
    build_external_provider, build_loaded_secrets_message, diff_secrets, load_secrets_from_path,
    process_env_lookup, refresh_interval_for, registered_external_kinds, resolve_source,
};
