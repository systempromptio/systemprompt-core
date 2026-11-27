//! Wire contract with a systemprompt instance, and nothing else: no IO, no
//! runtime, no HTTP client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// Errors of the pure helpers; none carries a credential or the value it
/// refused.
pub mod error;
/// The host id the harness presents and the host-targeting rules of manifest
/// entries.
pub mod host;
/// Typed identifiers, local and re-exported from `systemprompt-identifiers`.
pub mod ids;
/// RFC 8785 canonical JSON, the form the manifest payload is signed in.
pub mod jcs;
/// Derived names several crates must agree on byte for byte.
pub mod names;
/// URL safety rules: https or loopback HTTP, same-origin paths, where a bearer
/// may go.
pub mod origin;
/// Credential newtypes that redact in `Debug` and `Display` and zeroize on
/// drop.
pub mod secret;
/// The manifest version, its total order, and the envelope clock-skew rule.
pub mod version;
/// UI-facing state with no secrets in it.
pub mod view;
/// Every request and answer body the harness exchanges with an instance.
pub mod wire;

pub use error::{JwtError, OriginError, PubkeyError, SecretError};
pub use host::{HOST_HEADER_VALUE, HOST_ID};
pub use secret::{ClientSecret, DeviceCredential, HookToken, Jwt, OneTimeCode, Pat};
pub use version::{ManifestVersion, ManifestVersionParseError};
