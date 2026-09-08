//! Effective-access verification: does the unelevated user behind this
//! process hold Modify on a tree, evaluated with the linked (unelevated) token
//! when the process itself is elevated.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    AccessCheck, DACL_SECURITY_INFORMATION, DuplicateToken, GENERIC_MAPPING,
    GROUP_SECURITY_INFORMATION, GetTokenInformation, OWNER_SECURITY_INFORMATION,
    SecurityImpersonation, TOKEN_LINKED_TOKEN, TokenElevationType, TokenElevationTypeFull,
    TokenLinkedToken,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};

use super::{Descriptor, checked, process_token, status, wide};

fn unelevated_token() -> io::Result<OwnedHandle> {
    let token = process_token()?;
    let mut elevation = 0u32;
    let mut size = 0;
    // SAFETY: each output has the size of the selected token information class.
    unsafe {
        checked(GetTokenInformation(
            token.as_raw_handle(),
            TokenElevationType,
            (&raw mut elevation).cast(),
            size_of::<u32>() as u32,
            &raw mut size,
        ))?;
        let base = if elevation == TokenElevationTypeFull as u32 {
            let mut linked = TOKEN_LINKED_TOKEN {
                LinkedToken: null_mut(),
            };
            checked(GetTokenInformation(
                token.as_raw_handle(),
                TokenLinkedToken,
                (&raw mut linked).cast(),
                size_of::<TOKEN_LINKED_TOKEN>() as u32,
                &raw mut size,
            ))?;
            OwnedHandle::from_raw_handle(linked.LinkedToken)
        } else {
            token
        };
        let mut impersonation = null_mut();
        checked(DuplicateToken(
            base.as_raw_handle(),
            SecurityImpersonation,
            &raw mut impersonation,
        ))?;
        Ok(OwnedHandle::from_raw_handle(impersonation))
    }
}
pub fn verify_modify_tree(path: &Path) -> io::Result<()> {
    let token = unelevated_token()?;
    verify_modify(path, &token)?;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "refusing org-plugins link {}",
            path.display()
        )));
    }
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            verify_modify_tree(&entry?.path())?;
        }
    }
    Ok(())
}
fn verify_modify(path: &Path, token: &OwnedHandle) -> io::Result<()> {
    let path_wide = wide(path)?;
    let mut descriptor = null_mut();
    // SAFETY: outputs are valid and descriptor is freed after AccessCheck.
    unsafe {
        status(GetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
            &raw mut descriptor,
        ))?;
        let descriptor = Descriptor(descriptor);
        let mapping = GENERIC_MAPPING {
            GenericRead: FILE_GENERIC_READ,
            GenericWrite: FILE_GENERIC_WRITE,
            GenericExecute: FILE_GENERIC_EXECUTE,
            GenericAll: FILE_ALL_ACCESS,
        };
        let desired = FILE_GENERIC_READ | FILE_GENERIC_WRITE | FILE_GENERIC_EXECUTE | DELETE;
        let mut privileges = vec![0usize; 128];
        let mut length = (privileges.len() * size_of::<usize>()) as u32;
        let (mut granted, mut allowed) = (0, 0);
        checked(AccessCheck(
            descriptor.0,
            token.as_raw_handle(),
            desired,
            &raw const mapping,
            privileges.as_mut_ptr().cast(),
            &raw mut length,
            &raw mut granted,
            &raw mut allowed,
        ))?;
        if allowed == 0 || granted & desired != desired {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "{}: intended unelevated user lacks Modify access",
                    path.display()
                ),
            ));
        }
    }
    Ok(())
}
