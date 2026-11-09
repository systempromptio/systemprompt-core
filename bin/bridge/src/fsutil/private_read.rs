//! Reading a private file, with the Windows DACL repair a denied read needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::{fs, io};

// Why: Windows reports a read of a directory as access denied, so a directory
// is named as one before the denial is treated as a DACL fault. The owner of a
// private file can rewrite a DACL that lost every entry, so a denied read of a
// file is repaired once and retried before it is reported.
#[cfg(target_os = "windows")]
pub fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            if fs::metadata(path).is_ok_and(|meta| meta.is_dir()) {
                return Err(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("{}: is a directory", path.display()),
                ));
            }
            let reader = crate::windows_acl::current_sid()?;
            crate::windows_acl::repair_private(path, &reader)?;
            fs::read(path).map_err(|e| super::io_context("read after DACL repair", e))
        },
        other => other,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn read_private(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}
