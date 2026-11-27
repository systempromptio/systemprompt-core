//! Local security primitives for a per-user desktop app.
//!
//! Secrets are sealed for the current user through a [`Sealer`] (DPAPI in the
//! current-user scope with the app id as entropy on Windows; AES-256-GCM under
//! a random key the login keychain holds for the app id on macOS; elsewhere
//! only an in-memory sealer for tests and development, which never writes to
//! disk) and kept as [`SealedString`] base64 text, or held in the OS credential
//! store (Credential Manager, the login keychain) through [`Keystore`]. Files
//! that hold them are written owner-only and atomically: staged beside the
//! target, flushed, renamed over it, restricted to the current user and read
//! back. The skill materialiser's writes and removals never go through a link
//! or outside their root.
//!
//! Every filesystem call here is blocking; async callers run them on a blocking
//! thread. `unsafe` is confined to the Windows FFI module; the macOS module
//! uses safe bindings only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![deny(unsafe_code)]

mod compare;
mod error;
mod files;
mod keystore;
#[cfg(target_os = "macos")]
mod mac;
mod sealed;
mod sealer;
#[cfg(windows)]
mod win;

pub use compare::constant_time_eq;
pub use error::SecureError;
pub use files::{
    Access, WriteOutcome, contained_path, ensure_private_dir, read_optional, remove_if_exists,
    write_atomic, write_if_changed, write_private, write_shared,
};
pub use keystore::Keystore;
#[cfg(target_os = "macos")]
pub use mac::KeychainSealer;
pub use sealed::SealedString;
#[cfg(windows)]
pub use sealer::DpapiSealer;
pub use sealer::{MemorySealer, Sealer, platform_sealer};
pub use zeroize::{Zeroize, Zeroizing};
