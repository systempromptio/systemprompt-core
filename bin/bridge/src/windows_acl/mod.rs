//! Windows private-file descriptors and effective access verification.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![allow(unsafe_code, reason = "Windows descriptor and token APIs require FFI")]

mod access;
mod describe;

pub(crate) use self::access::verify_modify_tree;
pub(crate) use self::describe::describe;

use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SE_FILE_OBJECT, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    GetTokenInformation, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_DUPLICATE, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, READ_CONTROL, WRITE_DAC,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(super) struct Descriptor(pub(super) PSECURITY_DESCRIPTOR);
impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: this allocation came from an API requiring LocalFree.
        unsafe {
            LocalFree(self.0);
        }
    }
}

pub(super) fn checked(value: i32) -> io::Result<()> {
    if value == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
pub(super) fn status(value: u32) -> io::Result<()> {
    if value == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(value.cast_signed()))
    }
}
pub(super) fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(io::Error::other("path contains NUL"));
    }
    value.push(0);
    Ok(value)
}
pub(super) fn process_token() -> io::Result<OwnedHandle> {
    let mut token = null_mut();
    // SAFETY: output points to a live handle slot; the returned token is owned.
    unsafe {
        checked(OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &raw mut token,
        ))?;
        Ok(OwnedHandle::from_raw_handle(token))
    }
}
pub(crate) fn current_sid() -> io::Result<String> {
    let token = process_token()?;
    let mut length = 0;
    // SAFETY: the first query asks for the required size with no output buffer.
    let sized = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            null_mut(),
            0,
            &raw mut length,
        )
    };
    if sized != 0
        || io::Error::last_os_error().raw_os_error()
            != Some(windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER as i32)
        || length == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut storage = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    let mut text = null_mut();
    // SAFETY: storage is aligned and sized for TOKEN_USER; SID remains live until
    // conversion ends.
    unsafe {
        checked(GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &raw mut length,
        ))?;
        let user = &*storage.as_ptr().cast::<TOKEN_USER>();
        checked(ConvertSidToStringSidW(user.User.Sid, &raw mut text))?;
        let allocation = Descriptor(text.cast());
        let mut n = 0;
        while *text.add(n) != 0 {
            n += 1;
        }
        let result =
            String::from_utf16(std::slice::from_raw_parts(text, n)).map_err(io::Error::other);
        drop(allocation);
        result
    }
}
fn private_descriptor(reader: &str) -> io::Result<Descriptor> {
    if !reader.starts_with("S-1-")
        || !reader
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
    {
        return Err(io::Error::other("invalid private-file reader SID"));
    }
    let sddl: Vec<u16> = format!("D:P(A;;FA;;;{reader})(A;;FA;;;SY)(A;;FA;;;BA)")
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
pub(crate) fn create_private(path: &Path, reader: &str) -> io::Result<File> {
    let descriptor = private_descriptor(reader)?;
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
    let expected = private_descriptor(reader)?;
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
    let mut expected_acl = null_mut();
    let (mut present, mut defaulted) = (0, 0);
    // SAFETY: `expected` is a live self-relative descriptor built above and
    // the out-params are live locals.
    unsafe {
        checked(GetSecurityDescriptorDacl(
            expected.0,
            &raw mut present,
            &raw mut expected_acl,
            &raw mut defaulted,
        ))?;
    }
    let (mut control, mut revision) = (0, 0);
    // SAFETY: `actual` owns the descriptor GetSecurityInfo allocated.
    unsafe {
        checked(GetSecurityDescriptorControl(
            actual.0,
            &raw mut control,
            &raw mut revision,
        ))?;
    }
    if actual_acl.is_null() || expected_acl.is_null() || control & SE_DACL_PROTECTED == 0 {
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

pub(crate) fn protect_directory(path: &Path) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "private directory {} is a link",
            path.display()
        )));
    }
    let reader = current_sid()?;
    let descriptor = private_descriptor(&reader)?;
    let file = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let (mut present, mut defaulted) = (0, 0);
    let mut acl = null_mut();
    // SAFETY: descriptor and directory handle remain live throughout both calls.
    unsafe {
        checked(GetSecurityDescriptorDacl(
            descriptor.0,
            &raw mut present,
            &raw mut acl,
            &raw mut defaulted,
        ))?;
        if present == 0 || acl.is_null() {
            return Err(io::Error::other("private directory ACL missing"));
        }
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
    verify_private(&file, &reader)
}
