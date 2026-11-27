//! Wire contract with a systemprompt instance, and nothing else: no IO, no
//! runtime, no HTTP client.
//!
//! Request and answer bodies are `systemprompt-models` types, header names are
//! `systemprompt_identifiers::headers` and route paths are
//! `systemprompt_models::ApiPaths`; this module holds what only the device
//! needs beside them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Errors of the pure helpers; none carries a credential or the value it
/// refused.
pub mod error;
/// The device's own typed identifiers.
pub mod ids;
/// RFC 8785 canonical JSON, the form the manifest payload is signed in.
pub mod jcs;
mod lenient;
/// The device's reading of the signed manifest.
pub mod manifest;
/// Derived names several crates must agree on byte for byte.
pub mod names;
/// URL safety rules: https or loopback HTTP, same-origin paths, where a bearer
/// may go.
pub mod origin;
/// Route paths `ApiPaths` does not name yet.
pub mod paths;
/// The installation receipt built from a plan and its readback.
pub mod receipts;
/// Credential newtypes that redact in `Debug` and `Display` and zeroize on
/// drop.
pub mod secret;
/// The host the device presents and the host-targeting rules of manifest
/// entries.
pub mod targets;
/// The acceptance rules that only need a manifest version and a clock.
pub mod version;
/// Answer bodies that carry a credential.
pub mod wire;

pub use error::{JwtError, OriginError, PubkeyError, SecretError};
pub use secret::{ClientSecret, DeviceCredential, HookToken, Jwt, OneTimeCode, Pat};
