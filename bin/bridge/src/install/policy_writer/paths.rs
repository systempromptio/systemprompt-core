//! Directory protection anchored at the operating system's machine-data root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::File;
use std::io;

use super::{BIN_SDDL, INBOX_SDDL, Layout, OUTBOX_SDDL};

pub(super) fn secure_directories(layout: &Layout, create: bool) -> io::Result<Vec<File>> {
    let program_data = crate::windows_acl::program_data()?;
    if *layout != Layout::under(&program_data) {
        return Err(io::Error::other(
            "writer layout does not use machine ProgramData",
        ));
    }
    let brand = layout
        .root
        .parent()
        .ok_or_else(|| io::Error::other("missing brand directory"))?;
    let mut handles = vec![crate::windows_acl::lock_machine_path(&program_data, true)?];
    for (path, sddl) in [
        (brand, BIN_SDDL),
        (layout.root.as_path(), BIN_SDDL),
        (layout.bin.as_path(), BIN_SDDL),
        (layout.inbox.as_path(), INBOX_SDDL),
        (layout.outbox.as_path(), OUTBOX_SDDL),
    ] {
        if create {
            match std::fs::create_dir(path) {
                Ok(()) => {},
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {},
                Err(e) => return Err(e),
            }
        }
        handles.push(crate::windows_acl::lock_machine_path(path, true)?);
        if create {
            crate::windows_acl::apply_directory_sddl(path, sddl)?;
        } else {
            crate::windows_acl::verify_directory_sddl(path, sddl)?;
        }
    }
    Ok(handles)
}
