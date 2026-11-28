//! Files written atomically and read back: a temporary file in the same
//! directory, flushed, renamed over the target, then compared with what was
//! asked. Private files are restricted to the current user (an explicit
//! protected DACL on Windows, mode 0600 and the user's ownership elsewhere)
//! from the moment they exist.
//! Nothing is written through a symlink or reparse point, and
//! [`contained_path`] keeps a manifest-supplied relative path inside its root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod contained;
mod write;

pub(crate) use contained::check_segment;
pub use contained::contained_path;
pub use write::{
    ensure_private_dir, read_optional, remove_if_exists, write_atomic, write_if_changed,
    write_private, write_shared,
};

/// Who may read a written file. `Private` is the current user only (and, on
/// Windows, SYSTEM and Administrators); `Shared` is whatever the directory
/// grants by inheritance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Private,
    Shared,
}

/// Whether [`write_if_changed`] touched the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum WriteOutcome {
    Written,
    Unchanged,
}
