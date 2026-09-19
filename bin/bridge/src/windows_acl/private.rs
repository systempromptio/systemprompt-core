//! Private-file descriptors: creation, verification, directory protection and
//! owner repair.
//!
//! A protected DACL on a *directory* is propagated by Windows to every
//! unprotected child: their inherited entries are dropped and replaced by the
//! parent's inheritable ones. A directory descriptor with no inheritable
//! entries therefore leaves such children with an empty DACL that denies even
//! their owner, which is how the loopback key minted by an older release
//! became unreadable after an upgrade. Directory descriptors carry `OICI` so
//! propagation grants the same three principals instead of nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::File;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::ptr::null_mut;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SE_FILE_OBJECT,
    SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, READ_CONTROL, WRITE_DAC,
};

use super::{Descriptor, checked, current_sid, status, wide};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    File,
    Directory,
}

pub(super) fn private_descriptor(reader: &str, scope: Scope) -> io::Result<Descriptor> {
    if !reader.starts_with("S-1-")
        || !reader
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
    {
        return Err(io::Error::other("invalid private-file reader SID"));
    }
    let flags = match scope {
        Scope::File => "",
        Scope::Directory => "OICI",
    };
    descriptor_from_sddl(&format!(
        "D:P(A;{flags};FA;;;{reader})(A;{flags};FA;;;SY)(A;{flags};FA;;;BA)"
    ))
}

pub(super) fn descriptor_from_sddl(sddl: &str) -> io::Result<Descriptor> {
    let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = null_mut();
    // SAFETY: SDDL is NUL terminated and output lives through this call.
    unsafe {
        checked(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &raw mut descriptor,
            null_mut(),
        ))?;
    }
    Ok(Descriptor(descriptor))
}

// Why: the policy writer's spool and binary directories carry DACLs the
// private-file shape cannot express (users may add a request, never read
// another's); the descriptor is the caller's SDDL, applied protected and
// verified byte for byte the way a private file is.
pub(crate) fn apply_directory_sddl(path: &Path, sddl: &str) -> io::Result<()> {
    let expected = descriptor_from_sddl(sddl)?;
    let file = open_for_dac(path, Scope::Directory, READ_CONTROL | WRITE_DAC)?;
    if verify_against(&file, &expected).is_ok() {
        return Ok(());
    }
    let _guard = PROTECT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let acl = descriptor_dacl(&expected)?;
    // SAFETY: descriptor and handle remain live throughout the call.
    unsafe {
        status(SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null_mut(),
        ))?;
    }
    verify_against(&file, &expected)
}

pub(crate) fn verify_directory_sddl(path: &Path, sddl: &str) -> io::Result<()> {
    let expected = descriptor_from_sddl(sddl)?;
    let file = open_for_dac(path, Scope::Directory, READ_CONTROL)?;
    verify_against(&file, &expected)
}

fn verify_against(file: &File, expected: &Descriptor) -> io::Result<()> {
    let mut actual = null_mut();
    let mut actual_acl = null_mut();
    // SAFETY: file handle is live; Windows allocates a descriptor owned by
    // Descriptor.
    unsafe {
        status(GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            &raw mut actual_acl,
            null_mut(),
            &raw mut actual,
        ))?;
    }
    let actual = Descriptor(actual);
    compare_dacl(&actual, actual_acl, expected)
}

pub(super) fn descriptor_dacl(descriptor: &Descriptor) -> io::Result<*mut ACL> {
    let (mut present, mut defaulted) = (0, 0);
    let mut acl = null_mut();
    // SAFETY: the descriptor is a live self-relative descriptor and the
    // out-params are live locals.
    unsafe {
        checked(GetSecurityDescriptorDacl(
            descriptor.0,
            &raw mut present,
            &raw mut acl,
            &raw mut defaulted,
        ))?;
    }
    if present == 0 || acl.is_null() {
        return Err(io::Error::other("private descriptor ACL missing"));
    }
    Ok(acl)
}

pub(crate) fn create_private(path: &Path, reader: &str) -> io::Result<File> {
    let descriptor = private_descriptor(reader, Scope::File)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let path = wide(path)?;
    // SAFETY: attributes and path remain valid; CREATE_NEW never follows an
    // existing destination.
    unsafe {
        let handle = CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            &raw const attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let file = File::from_raw_handle(handle);
        verify_private(&file, reader)?;
        Ok(file)
    }
}

pub(crate) fn verify_private(file: &File, reader: &str) -> io::Result<()> {
    verify_scope(file, reader, Scope::File)
}

fn verify_scope(file: &File, reader: &str, scope: Scope) -> io::Result<()> {
    verify_against(file, &private_descriptor(reader, scope)?)
}

pub(super) fn compare_dacl(
    actual: &Descriptor,
    actual_acl: *mut ACL,
    expected: &Descriptor,
) -> io::Result<()> {
    let expected_acl = descriptor_dacl(expected)?;
    let (mut control, mut revision) = (0, 0);
    // SAFETY: `actual` owns a live descriptor.
    unsafe {
        checked(GetSecurityDescriptorControl(
            actual.0,
            &raw mut control,
            &raw mut revision,
        ))?;
    }
    if actual_acl.is_null() || control & SE_DACL_PROTECTED == 0 {
        return Err(io::Error::other(
            "private file has absent or inheritable DACL",
        ));
    }
    // SAFETY: both ACL pointers were checked non-null above and each points
    // at an ACL whose header carries its own byte length.
    let (actual_bytes, expected_bytes) = unsafe {
        (
            std::slice::from_raw_parts(actual_acl.cast::<u8>(), usize::from((*actual_acl).AclSize)),
            std::slice::from_raw_parts(
                expected_acl.cast::<u8>(),
                usize::from((*expected_acl).AclSize),
            ),
        )
    };
    if actual_bytes != expected_bytes {
        return Err(io::Error::other(
            "DACL did not match the expected protected descriptor",
        ));
    }
    Ok(())
}

fn set_dacl(file: &File, reader: &str, scope: Scope) -> io::Result<()> {
    let descriptor = private_descriptor(reader, scope)?;
    let acl = descriptor_dacl(&descriptor)?;
    // SAFETY: descriptor and handle remain live throughout the call.
    unsafe {
        status(SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null_mut(),
        ))
    }
}

fn open_for_dac(path: &Path, scope: Scope, access: u32) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.access_mode(access);
    match scope {
        Scope::File => options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT),
        Scope::Directory => {
            options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        },
    };
    let file = options.open(path)?;
    if file.metadata()?.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "private path {} is a link",
            path.display()
        )));
    }
    Ok(file)
}

// Why: SetSecurityInfo with a protected DACL on a directory re-propagates to
// every child, and a sibling thread mid `CreateFileW`/`MoveFileEx` inside that
// directory observes ACCESS_DENIED for the duration. Callers share brand-level
// directories (the temp dir every generated profile lands in), so the DACL is
// rewritten only when verification fails, and that write is serialised.
static PROTECT: Mutex<()> = Mutex::new(());

pub(crate) fn protect_directory(path: &Path) -> io::Result<()> {
    let reader = current_sid()?;
    let probe = open_for_dac(path, Scope::Directory, READ_CONTROL)?;
    if verify_scope(&probe, &reader, Scope::Directory).is_ok() {
        return Ok(());
    }
    drop(probe);
    let _guard = PROTECT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let file = open_for_dac(path, Scope::Directory, READ_CONTROL | WRITE_DAC)?;
    if verify_scope(&file, &reader, Scope::Directory).is_ok() {
        return Ok(());
    }
    set_dacl(&file, &reader, Scope::Directory)?;
    verify_scope(&file, &reader, Scope::Directory)
}
