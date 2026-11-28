//! Owner-only security descriptors: creation of a private file with its DACL
//! already applied, verification of a file's DACL against the expected one, and
//! protection of a directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "security descriptor and file creation APIs are raw Win32 FFI"
)]

use std::fs::File;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, SE_FILE_OBJECT,
    SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
    SECURITY_ATTRIBUTES,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    READ_CONTROL, WRITE_DAC,
};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, INFINITE, ReleaseMutex, WaitForSingleObject,
};

use super::ffi::{LocalAllocation, checked, status, wide};
use super::filesystem::is_reparse_point;
use super::identity::current_sid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    File,
    Directory,
}

fn private_descriptor(owner: &str, scope: Scope) -> io::Result<LocalAllocation> {
    if !owner.starts_with("S-1-")
        || !owner
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || b == b'S')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid owner SID",
        ));
    }
    let flags = match scope {
        Scope::File => "",
        Scope::Directory => "OICI",
    };
    let sddl = format!("D:P(A;{flags};FA;;;{owner})(A;{flags};FA;;;SY)(A;{flags};FA;;;BA)");
    let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: the SDDL is NUL terminated and the output slot is a live local; the
    // descriptor Windows allocates is owned by the returned LocalAllocation.
    unsafe {
        checked(ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &raw mut descriptor,
            null_mut(),
        ))?;
    }
    Ok(LocalAllocation(descriptor))
}

fn descriptor_dacl(descriptor: &LocalAllocation) -> io::Result<*mut ACL> {
    let (mut present, mut defaulted) = (0, 0);
    let mut acl = null_mut();
    // SAFETY: the descriptor is a live self-relative descriptor and every output is
    // a live local.
    unsafe {
        checked(GetSecurityDescriptorDacl(
            descriptor.0,
            &raw mut present,
            &raw mut acl,
            &raw mut defaulted,
        ))?;
    }
    if present == 0 || acl.is_null() {
        return Err(io::Error::other("private descriptor has no DACL"));
    }
    Ok(acl)
}

pub(crate) fn create_private(path: &Path, owner: &str) -> io::Result<File> {
    let descriptor = private_descriptor(owner, Scope::File)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let path = wide(path)?;
    // SAFETY: the path and the attributes (and the descriptor they point at)
    // outlive the call; CREATE_NEW fails rather than open or follow anything
    // that already exists. A valid handle is owned by the returned File.
    let file = unsafe {
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
        File::from_raw_handle(handle)
    };
    verify_private(&file, owner)?;
    Ok(file)
}

pub(crate) fn verify_private(file: &File, owner: &str) -> io::Result<()> {
    verify_against(file, &private_descriptor(owner, Scope::File)?)
}

fn verify_against(file: &File, expected: &LocalAllocation) -> io::Result<()> {
    let mut actual = null_mut();
    let mut actual_acl = null_mut();
    // SAFETY: the handle is live and the outputs are live locals; Windows allocates
    // the descriptor, owned by LocalAllocation below, and `actual_acl` points
    // into it.
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
    let actual = LocalAllocation(actual);
    compare_dacl(&actual, actual_acl, expected)
}

fn compare_dacl(
    actual: &LocalAllocation,
    actual_acl: *mut ACL,
    expected: &LocalAllocation,
) -> io::Result<()> {
    let expected_acl = descriptor_dacl(expected)?;
    let (mut control, mut revision) = (0u16, 0u32);
    // SAFETY: `actual` owns a live descriptor and the outputs are live locals.
    unsafe {
        checked(GetSecurityDescriptorControl(
            actual.0,
            &raw mut control,
            &raw mut revision,
        ))?;
    }
    if actual_acl.is_null() || control & SE_DACL_PROTECTED == 0 {
        return Err(io::Error::other(
            "DACL is absent or inherits from the parent",
        ));
    }
    // SAFETY: both ACL pointers are non-null and point at ACLs inside descriptors
    // that are alive for this whole function; each ACL header carries its own
    // byte length.
    let same = unsafe {
        let actual_bytes =
            std::slice::from_raw_parts(actual_acl.cast::<u8>(), usize::from((*actual_acl).AclSize));
        let expected_bytes = std::slice::from_raw_parts(
            expected_acl.cast::<u8>(),
            usize::from((*expected_acl).AclSize),
        );
        actual_bytes == expected_bytes
    };
    if same {
        Ok(())
    } else {
        Err(io::Error::other(
            "DACL does not match the owner-only descriptor",
        ))
    }
}

fn open_directory_for_dac(path: &Path, access: u32) -> io::Result<File> {
    let file = std::fs::OpenOptions::new()
        .access_mode(access | FILE_READ_ATTRIBUTES)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = file.metadata()?;
    if is_reparse_point(&metadata) || !metadata.is_dir() {
        return Err(io::Error::other(
            "private directory is a link or not a directory",
        ));
    }
    Ok(file)
}

const DACL_WRITE_MUTEX: &str = "Local\\systemprompt-device-private-dacl";

// Why: a protected DACL set on a directory is re-propagated to every child, and
// a sibling thread or process creating or renaming inside that directory sees
// ACCESS_DENIED meanwhile; the DACL is therefore written only when verification
// fails, and that write is serialised on a named Windows mutex of the logon
// session, which the thread that acquired it must release.
struct DaclWriteGuard(OwnedHandle);

impl DaclWriteGuard {
    fn acquire() -> io::Result<Self> {
        let name: Vec<u16> = DACL_WRITE_MUTEX.encode_utf16().chain(Some(0)).collect();
        // SAFETY: the name is NUL terminated and outlives the call; a non-null
        // handle is a mutex handle owned by the OwnedHandle and closed on drop.
        let mutex = unsafe {
            let handle = CreateMutexW(null(), 0, name.as_ptr());
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            OwnedHandle::from_raw_handle(handle)
        };
        // SAFETY: the handle is a live mutex handle for the whole wait.
        let waited = unsafe { WaitForSingleObject(mutex.as_raw_handle(), INFINITE) };
        match waited {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Ok(Self(mutex)),
            _ => Err(io::Error::last_os_error()),
        }
    }
}

impl Drop for DaclWriteGuard {
    fn drop(&mut self) {
        // SAFETY: the handle is live until the field drops, and this thread
        // has owned the mutex since `acquire` returned on it.
        unsafe {
            ReleaseMutex(self.0.as_raw_handle());
        }
    }
}

pub(crate) fn protect_directory(path: &Path) -> io::Result<()> {
    let owner = current_sid()?;
    let expected = private_descriptor(&owner, Scope::Directory)?;
    let probe = open_directory_for_dac(path, READ_CONTROL)?;
    if verify_against(&probe, &expected).is_ok() {
        return Ok(());
    }
    drop(probe);
    let _guard = DaclWriteGuard::acquire()?;
    let file = open_directory_for_dac(path, READ_CONTROL | WRITE_DAC)?;
    if verify_against(&file, &expected).is_ok() {
        return Ok(());
    }
    let acl = descriptor_dacl(&expected)?;
    // SAFETY: the handle is live and `acl` points into `expected`, which outlives
    // the call.
    unsafe {
        status(SetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            acl,
            null(),
        ))?;
    }
    verify_against(&file, &expected)
}
