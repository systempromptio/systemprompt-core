//! Local security primitives for a per-user desktop app.
//!
//! Secrets are sealed for the current user through a [`Sealer`] and kept as
//! [`SealedString`] base64 text, or held in the OS credential store through
//! [`Keystore`]. Files that hold them are written owner-only and atomically:
//! staged beside the target, flushed, renamed over it, restricted to the
//! current user and read back. The skill materialiser's writes and removals
//! never go through a link or outside their root.
//!
//! What a sealer, a credential store and an owner-only file are on each system
//! is [`crate::platform`]'s; nothing here names a system.
//!
//! Every filesystem call here is blocking; async callers run them on a blocking
//! thread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![deny(unsafe_code)]

mod compare;
mod error;
mod files;
mod keystore;
mod sealed;
mod sealer;

pub use compare::constant_time_eq;
pub use error::SecureError;
pub(crate) use files::check_segment;
pub use files::{
    Access, WriteOutcome, contained_path, ensure_private_dir, read_optional, remove_if_exists,
    write_atomic, write_if_changed, write_private, write_shared,
};
pub use keystore::Keystore;
pub use sealed::SealedString;
pub use sealer::{MemorySealer, Sealer, platform_sealer};
pub use zeroize::{Zeroize, Zeroizing};
