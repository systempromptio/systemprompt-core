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
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SE_FILE_OBJECT, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED,
    SECURITY_ATTRIBUTES,
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

fn private_descriptor(reader: &str, scope: Scope) -> io::Result<Descriptor> {
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
    let sddl: Vec<u16> =
        format!("D:P(A;{flags};FA;;;{reader})(A;{flags};FA;;;SY)(A;{flags};FA;;;BA)")
            .encode_utf16()
            .chain(Some(0))
            .collect();
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

fn descriptor_dacl(descriptor: &Descriptor) -> io::Result<*mut ACL> {
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
    let expected = private_descriptor(reader, scope)?;
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
    let expected_acl = descriptor_dacl(&expected)?;
    let (mut control, mut revision) = (0, 0);
    // SAFETY: `actual` owns the descriptor GetSecurityInfo allocated.
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
            "private file DACL did not match requested reader, SYSTEM and Administrators",
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

// Why: a path-based metadata query opens the file itself, which an empty DACL
// refuses; the handle is opened first with only the owner-implicit rights and
// the reparse check is made through it.
fn open_for_dac(path: &Path, scope: Scope) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.access_mode(READ_CONTROL | WRITE_DAC);
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

pub(crate) fn protect_directory(path: &Path) -> io::Result<()> {
    let reader = current_sid()?;
    let file = open_for_dac(path, Scope::Directory)?;
    set_dacl(&file, &reader, Scope::Directory)?;
    verify_scope(&file, &reader, Scope::Directory)
}

fn owner_sid(file: &File) -> io::Result<String> {
    let mut descriptor = null_mut();
    let mut owner = null_mut();
    // SAFETY: the handle is live; the descriptor Windows allocates owns the
    // SID for as long as `Descriptor` lives.
    unsafe {
        status(GetSecurityInfo(
            file.as_raw_handle(),
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

// Why: an owner always holds READ_CONTROL and WRITE_DAC, so a file whose DACL
// no longer names them can still be repaired by them, and only by them.
pub(crate) fn repair_private(path: &Path, reader: &str) -> io::Result<()> {
    let before = super::describe(path).unwrap_or_else(|e| format!("<{e}>"));
    let file = open_for_dac(path, Scope::File).map_err(|e| step("open for WRITE_DAC", &e))?;
    let owner = owner_sid(&file).map_err(|e| step("read owner", &e))?;
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
    set_dacl(&file, reader, Scope::File).map_err(|e| step("set DACL", &e))?;
    verify_private(&file, reader).map_err(|e| step("verify DACL", &e))?;
    tracing::warn!(
        path = %path.display(),
        before = %before,
        after = %super::describe(path).unwrap_or_else(|e| format!("<{e}>")),
        "repaired the access control list of a private file this user owns"
    );
    Ok(())
}

fn step(action: &str, e: &io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("repair private file: {action}: {e}"))
}
