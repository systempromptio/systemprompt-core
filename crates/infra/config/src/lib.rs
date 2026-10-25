//! # systemprompt-config
//!
//! Profile-based configuration for systemprompt.io. This crate is the
//! bootstrap layer: it loads the active profile YAML and the matching
//! secrets document into owned values the entry crates hand to every other
//! layer (database, runtime, agent).
//!
//! ## Public surface
//!
//! - [`LoadedProfile`] / [`SecretsService`] — the active profile and the
//!   reloadable secrets document, built in that order by the entry-crate boot
//!   sequence.
//! - [`build_from_profile`] — builds a runtime
//!   [`systemprompt_manifest::Config`] from the active profile.
//! - [`ProviderCatalogService`], [`SecurityConfigService`] — typed mutations of
//!   the services provider registry and the profile's security section, backing
//!   the `admin config catalog` / `admin config security` CLI surfaces.
//! - [`SkillConfigValidator`] — `DomainConfig` implementation that walks
//!   `skills/` and reports missing or malformed manifests.
//!
//! ## Errors
//!
//! All public APIs return [`ConfigResult<T>`] (i.e.
//! `Result<T, ConfigError>`). [`ConfigError`] composes the bootstrap,
//! profile, secrets, schema-validation, and lower-level
//! `serde`/`std::io` errors via `#[from]` so callers can use `?`
//! transparently.
//!
//! ## Feature flags
//!
//! This crate has no Cargo features — every dependency is required at
//! compile time. The `[package.metadata.docs.rs]` section in
//! `Cargo.toml` enables `all-features = true` for parity with the
//! rest of the workspace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bootstrap;
pub(crate) mod config_loader;
pub mod error;
pub mod fingerprint;
pub mod path_validation;
pub mod paths;
pub mod private_file;
pub(crate) mod services;
pub(crate) mod skill_validator;
pub mod state_dir;

pub use bootstrap::{
    BOOT_REQUIRED_SECRET_KEYS, DynSecretsProvider, ENCRYPTION_MASTER_KEY_BYTES, EnvLookup,
    KeyMaterialError, LoadedProfile, MANIFEST_SIGNING_SEED_BYTES, PINNED_SECRET_KEYS,
    ProfileBootstrapError, ReloadOutcome, ResolvedSource, SIGNING_KEY_PEM, SecretsBootstrapError,
    SecretsDocument, SecretsFetch, SecretsProvider, SecretsService, SecretsSourceFactory,
    SecretsSourceRegistration, SecretsStatus, SharedSecretsProvider, VaultAttemptFailure,
    VaultError, VaultKvProvider, build_external_provider, build_loaded_secrets_message,
    decode_master_key, decode_seed, diff_secrets, generate_master_key, generate_seed,
    load_secrets_from_path, persist_seed, process_env_lookup, refresh_interval_for,
    registered_external_kinds, resolve_source,
};
pub use systemprompt_manifest::profile::ExternalSecretsConfig;

pub use config_loader::{build_from_profile, resolve_instance_id, validate_database_config};
pub use error::{ConfigError, ConfigResult};
pub use fingerprint::{ConfigFingerprint, FINGERPRINT_HEX_LEN};
#[doc(hidden)]
pub use inventory as __inventory;
pub use paths::{
    AppPaths, BuildPaths, PathError, StoragePaths, SystemPaths, WebPaths, WritableRoot,
};
pub use private_file::write_private_atomic;
pub use services::{
    ConfigValidationError, CredentialSpec, EndpointSpec, ModelSpec, ProviderCatalogService,
    ProviderSpec, SecurityChange, SecurityConfigService, SecurityUpdate, generate_schema,
    read_catalog, validate_config, validate_yaml_file, validate_yaml_str, write_catalog,
};
pub use skill_validator::SkillConfigValidator;
pub use state_dir::{StateDirError, StateDirsError, create_state_dir, ensure_state_dirs_writable};
