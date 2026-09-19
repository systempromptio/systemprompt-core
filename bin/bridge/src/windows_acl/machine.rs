//! Machine-owned paths used by elevated task executables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    IsWellKnownSid, OWNER_SECURITY_INFORMATION, WinBuiltinAdministratorsSid, WinLocalSystemSid,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_LIST_DIRECTORY, FILE_READ_DATA, FILE_SHARE_READ, FILE_SHARE_WRITE, READ_CONTROL,
};
use windows_sys::Win32::UI::Shell::{CSIDL_COMMON_APPDATA, SHGetFolderPathW};

use super::{Descriptor, status};

pub(crate) fn program_data() -> io::Result<PathBuf> {
    let mut path = [0u16; 260];
    // SAFETY: SHGetFolderPathW writes at most MAX_PATH characters to this buffer.
    let result = unsafe {
        SHGetFolderPathW(
            null_mut(),
            CSIDL_COMMON_APPDATA.cast_signed(),
            null_mut(),
            0,
            path.as_mut_ptr(),
        )
    };
    if result < 0 {
        return Err(io::Error::other(format!(
            "resolve machine ProgramData: HRESULT {result:#x}"
        )));
    }
    let length = path
        .iter()
        .position(|ch| *ch == 0)
        .ok_or_else(|| io::Error::other("unterminated ProgramData path"))?;
    let path = PathBuf::from(std::ffi::OsString::from_wide(&path[..length]));
    if !path.is_absolute() {
        return Err(io::Error::other("ProgramData is not an absolute path"));
    }
    Ok(path)
}

pub(crate) fn lock_machine_path(path: &Path, directory: bool) -> io::Result<File> {
    let file = open_read_guard(path, directory)?;
    verify_owner(&file)?;
    Ok(file)
}

pub fn open_read_guard(path: &Path, directory: bool) -> io::Result<File> {
    let access = if directory {
        FILE_LIST_DIRECTORY
    } else {
        FILE_READ_DATA
    };
    let file = std::fs::OpenOptions::new()
        .access_mode(READ_CONTROL | access)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let metadata = file.metadata()?;
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err(io::Error::other(format!(
            "{} is not a regular machine path",
            path.display()
        )));
    }
    Ok(file)
}

fn verify_owner(file: &File) -> io::Result<()> {
    let mut owner = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: the file handle and output slots are live; the returned descriptor
    // owns the SID.
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
        let _descriptor = Descriptor(descriptor);
        if owner.is_null()
            || (IsWellKnownSid(owner, WinLocalSystemSid) == 0
                && IsWellKnownSid(owner, WinBuiltinAdministratorsSid) == 0)
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "machine path owner must be SYSTEM or Administrators; remove untrusted pre-existing directories before installing",
            ));
        }
    }
    Ok(())
}
