//! Bootstrap entry points.
//!
//! [`ProfileBootstrap`] and [`SecretsBootstrap`] install the process-global
//! profile and secrets, in that order. The ordering is a runtime invariant of
//! the entry-crate boot sequence — which interleaves credential and routing
//! resolution between the two steps — rather than a type-state, so these
//! initialisers are called directly by the CLI/API runners.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod manifest;
mod profile;
mod secrets;

pub use manifest::{MANIFEST_SIGNING_SEED_BYTES, decode_seed, generate_seed, persist_seed};
pub use profile::{ProfileBootstrap, ProfileBootstrapError};
pub use secrets::{
    SecretsBootstrap, SecretsBootstrapError, build_loaded_secrets_message, load_secrets_from_path,
    log_secrets_issue, log_secrets_skip, log_secrets_warn,
};
