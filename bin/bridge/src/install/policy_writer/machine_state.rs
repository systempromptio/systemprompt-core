//! The SYSTEM-owned records under the writer root: the binding the elevated
//! install writes and the last envelope version the writer accepted. Both
//! are written by an elevated process only, owned by SYSTEM or
//! Administrators, and protected with the binary's descriptor (users read,
//! never write); each is refused on read unless its owner and descriptor
//! still say so.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]

use std::io::{self, Read};
use std::path::Path;

use super::{AcceptedEnvelope, BIN_SDDL, Layout, MachineBinding, PolicyWriterError};

const MAX_RECORD_BYTES: u64 = 16 * 1024;

fn io_ctx(context: String) -> impl FnOnce(io::Error) -> PolicyWriterError {
    move |source| PolicyWriterError::Io { context, source }
}

fn read_protected(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let file = match crate::windows_acl::lock_machine_path(path, false) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    crate::windows_acl::verify_directory_sddl(path, BIN_SDDL)?;
    let mut bytes = Vec::new();
    (&file).take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(io::Error::other("machine record exceeds the size limit"));
    }
    Ok(Some(bytes))
}

fn write_protected(path: &Path, bytes: &[u8]) -> io::Result<()> {
    crate::fsutil::atomic_write_0644(path, bytes)?;
    let _file = crate::windows_acl::lock_machine_path(path, false)?;
    crate::windows_acl::apply_directory_sddl(path, BIN_SDDL)?;
    match read_protected(path)? {
        Some(back) if back == bytes => Ok(()),
        _ => Err(io::Error::other(format!(
            "{} does not read back as written",
            path.display()
        ))),
    }
}

pub(super) fn read_binding(layout: &Layout) -> Result<MachineBinding, PolicyWriterError> {
    let path = layout.binding_path();
    let bytes = read_protected(&path)
        .map_err(io_ctx(format!("read {}", path.display())))?
        .ok_or(PolicyWriterError::NoBinding)?;
    MachineBinding::decode(&bytes)
}

pub(super) fn write_binding(
    layout: &Layout,
    binding: &MachineBinding,
) -> Result<(), PolicyWriterError> {
    let path = layout.binding_path();
    let bytes = serde_json::to_vec(binding)
        .map_err(|e| io_ctx("encode the machine policy binding".to_owned())(io::Error::other(e)))?;
    write_protected(&path, &bytes).map_err(io_ctx(format!("write {}", path.display())))
}

pub(super) fn read_accepted(
    layout: &Layout,
) -> Result<Option<AcceptedEnvelope>, PolicyWriterError> {
    let path = layout.accepted_path();
    let Some(bytes) = read_protected(&path).map_err(io_ctx(format!("read {}", path.display())))?
    else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| io_ctx(format!("decode {}", path.display()))(io::Error::other(e)))
}

pub(super) fn write_accepted(
    layout: &Layout,
    accepted: &AcceptedEnvelope,
) -> Result<(), PolicyWriterError> {
    let path = layout.accepted_path();
    let bytes = serde_json::to_vec(accepted)
        .map_err(|e| io_ctx("encode the accepted envelope".to_owned())(io::Error::other(e)))?;
    write_protected(&path, &bytes).map_err(io_ctx(format!("write {}", path.display())))
}
