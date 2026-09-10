//! Owner repair of a private file whose DACL no longer names anyone.
//!
//! An empty DACL refuses every handle open, including one that asks only for
//! `WRITE_DAC`, while the named security functions reach the descriptor the
//! way `icacls` does. Everything here is therefore path-based: the reparse
//! check reads attributes without a handle, the owner and the DACL are read
//! and written by name, and the result is read back by name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSID,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
};

use super::private::{Scope, compare_dacl, descriptor_dacl, private_descriptor};
use super::{Descriptor, checked, status, wide};

pub(crate) fn repair_private(path: &Path, reader: &str) -> io::Result<()> {
    let before = super::describe(path).unwrap_or_else(|e| format!("<{e}>"));
    let path_w = wide(path)?;
    refuse_reparse_point(path, &path_w).map_err(|e| step("check attributes", &e))?;
    let owner = owner_sid(&path_w).map_err(|e| step("read owner", &e))?;
    if owner != reader {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} is owned by {owner}, not this user ({reader}); it cannot be repaired from \
                 this account",
                path.display()
            ),
        ));
    }
    let expected = private_descriptor(reader, Scope::File)?;
    let acl = descriptor_dacl(&expected)?;
    // SAFETY: the path is NUL terminated and `acl` lives inside `expected`,
    // which outlives the call.
    unsafe {
        status(SetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null_mut(),
        ))
        .map_err(|e| step("set DACL", &e))?;
    }
    verify_named(&path_w, &expected).map_err(|e| step("verify DACL", &e))?;
    tracing::warn!(
        path = %path.display(),
        before = %before,
        after = %super::describe(path).unwrap_or_else(|e| format!("<{e}>")),
        "repaired the access control list of a private file this user owns"
    );
    Ok(())
}

fn refuse_reparse_point(path: &Path, path_w: &[u16]) -> io::Result<()> {
    // SAFETY: the path is NUL terminated.
    let attributes = unsafe { GetFileAttributesW(path_w.as_ptr()) };
    if attributes == INVALID_FILE_ATTRIBUTES {
        return Err(io::Error::last_os_error());
    }
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::other(format!(
            "private path {} is a link",
            path.display()
        )));
    }
    Ok(())
}

fn owner_sid(path_w: &[u16]) -> io::Result<String> {
    let mut descriptor = null_mut();
    let mut owner: PSID = null_mut();
    // SAFETY: the path is NUL terminated; the descriptor Windows allocates owns
    // the SID for as long as `Descriptor` lives.
    unsafe {
        status(GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &raw mut owner,
            null_mut(),
            null_mut(),
            null_mut(),
            &raw mut descriptor,
        ))?;
        let descriptor = Descriptor(descriptor);
        let mut text = null_mut();
        checked(ConvertSidToStringSidW(owner, &raw mut text))?;
        let allocation = Descriptor(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(io::Error::other);
        drop(allocation);
        drop(descriptor);
        result
    }
}

fn verify_named(path_w: &[u16], expected: &Descriptor) -> io::Result<()> {
    let mut descriptor = null_mut();
    let mut acl: *mut ACL = null_mut();
    // SAFETY: the path is NUL terminated; the descriptor Windows allocates is
    // owned by `Descriptor` and `acl` points into it.
    unsafe {
        status(GetNamedSecurityInfoW(
            path_w.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &raw mut acl,
            null_mut(),
            &raw mut descriptor,
        ))?;
    }
    let actual = Descriptor(descriptor);
    compare_dacl(&actual, acl, expected)
}

fn step(action: &str, e: &io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("repair private file: {action}: {e}"))
}
