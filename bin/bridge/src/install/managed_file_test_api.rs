//! Delegating seam over the managed-file writer so the separate test
//! workspace can drive its escalation and removal arms.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::Path;

pub use super::ManagedWrite;

pub fn write_managed_file(path: &Path, bytes: &[u8], prompt: &str) -> io::Result<ManagedWrite> {
    super::write_managed_file(path, bytes, prompt)
}

pub fn remove_managed_file(path: &Path, prompt: &str) -> io::Result<bool> {
    super::remove_managed_file(path, prompt)
}
