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
    TokenElevationTypeLimited, TokenLinkedToken,
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
};

use super::{Descriptor, checked, process_token, status, wide};

pub(crate) fn elevation_summary() -> io::Result<String> {
    let token = process_token()?;
    let mut elevation = 0u32;
    let mut size = 0;
    // SAFETY: the output has the size of the selected token information class.
    unsafe {
        checked(GetTokenInformation(
            token.as_raw_handle(),
            TokenElevationType,
            (&raw mut elevation).cast(),
            size_of::<u32>() as u32,
            &raw mut size,
        ))?;
    }
    let kind = if elevation == TokenElevationTypeFull as u32 {
        "elevated (full administrator token)"
    } else if elevation == TokenElevationTypeLimited as u32 {
        "not elevated (limited token; UAC available)"
    } else {
        "default token (no UAC split)"
    };
    Ok(format!("{kind}; sid {}", super::current_sid()?))
}

fn unelevated_token() -> io::Result<OwnedHandle> {
    let token = process_token()?;
    let mut elevation = 0u32;
    let mut size = 0;
    // SAFETY: each output has the size of the selected token information class.
    unsafe {
        step(
            GetTokenInformation(
                token.as_raw_handle(),
                TokenElevationType,
                (&raw mut elevation).cast(),
                size_of::<u32>() as u32,
                &raw mut size,
            ),
            "GetTokenInformation(TokenElevationType)",
        )?;
        if elevation == TokenElevationTypeFull as u32 {
            // Why: the linked token comes back as an impersonation token at
            // SecurityIdentification level for any caller without SeTcb (an
            // elevated administrator included). AccessCheck accepts that
            // level as-is; asking DuplicateToken for SecurityImpersonation
            // requests a higher level than the source and fails with
            // ERROR_BAD_IMPERSONATION_LEVEL (1346).
            let mut linked = TOKEN_LINKED_TOKEN {
                LinkedToken: null_mut(),
            };
            step(
                GetTokenInformation(
                    token.as_raw_handle(),
                    TokenLinkedToken,
                    (&raw mut linked).cast(),
                    size_of::<TOKEN_LINKED_TOKEN>() as u32,
                    &raw mut size,
                ),
                "GetTokenInformation(TokenLinkedToken)",
            )?;
            return Ok(OwnedHandle::from_raw_handle(linked.LinkedToken));
        }
        let mut impersonation = null_mut();
        step(
            DuplicateToken(
                token.as_raw_handle(),
                SecurityImpersonation,
                &raw mut impersonation,
            ),
            "DuplicateToken(SecurityImpersonation)",
        )?;
        Ok(OwnedHandle::from_raw_handle(impersonation))
    }
}
fn step(value: i32, name: &str) -> io::Result<()> {
    checked(value).map_err(|e| io::Error::new(e.kind(), format!("{name}: {e}")))
}
pub(crate) fn verify_modify_tree(path: &Path) -> io::Result<()> {
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
        ))
        .map_err(|e| io::Error::new(e.kind(), format!("GetNamedSecurityInfoW: {e}")))?;
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
        step(
            AccessCheck(
                descriptor.0,
                token.as_raw_handle(),
                desired,
                &raw const mapping,
                privileges.as_mut_ptr().cast(),
                &raw mut length,
                &raw mut granted,
                &raw mut allowed,
            ),
            "AccessCheck",
        )?;
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
