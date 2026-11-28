//! Reparse-point detection and the replacing rename that keeps the moved file's
//! own security descriptor.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(unsafe_code, reason = "MoveFileExW is raw Win32 FFI")]

use std::fs::Metadata;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{
    ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
};

use super::ffi::{checked, wide};

pub(crate) fn is_reparse_point(metadata: &Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

// Why: `MoveFileExW` keeps the moved file's own security descriptor, where
// `ReplaceFileW` would copy the replaced file's; MOVEFILE_WRITE_THROUGH returns
// only once the rename is on disk.
pub(crate) fn move_replace(from: &Path, to: &Path) -> io::Result<()> {
    let from = wide(from)?;
    let to = wide(to)?;
    // SAFETY: both paths are NUL-terminated wide strings that outlive the call.
    unsafe {
        checked(MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        ))
    }
}

// Why: Windows refuses the replace with these codes while another handle
// (antivirus, the search indexer) holds the target open.
pub(crate) fn is_transient_replace_error(error: &io::Error) -> bool {
    error.raw_os_error().is_some_and(|code| {
        code == ERROR_ACCESS_DENIED.cast_signed()
            || code == ERROR_SHARING_VIOLATION.cast_signed()
            || code == ERROR_LOCK_VIOLATION.cast_signed()
    })
}
