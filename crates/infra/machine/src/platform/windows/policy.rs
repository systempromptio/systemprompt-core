//! The machine policy key and the policy file under `ProgramData`, read only.
//!
//! `ProgramData` lets any user create a directory, so the file is accepted
//! only when an administrator or the system owns it: a file a user put there,
//! or put back after deleting the real one, is theirs and is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![expect(
    unsafe_code,
    reason = "registry reads and the owner of a file are raw Win32 FFI"
)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io::{self, Read as _};
use std::os::windows::io::AsRawHandle as _;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, LocalFree};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    IsWellKnownSid, OWNER_SECURITY_INFORMATION, WinBuiltinAdministratorsSid, WinLocalSystemSid,
};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_DWORD, REG_EXPAND_SZ, REG_SZ,
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, RegQueryInfoKeyW,
};

use super::ffi;
use crate::policy::{
    DOCUMENT_LIMIT, PolicyContent, PolicyError, PolicyIdentity, PolicyLocation, PolicyRead,
    PolicyValue,
};

const POLICIES: &str = r"SOFTWARE\Policies";

const MACHINE_HIVE: &str = "HKLM";

struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle is an open registry key this value owns and
        // closes exactly once.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

struct RawValue {
    name: String,
    kind: u32,
    data: Vec<u8>,
}

fn status(value: u32) -> io::Result<()> {
    if value == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(value.cast_signed()))
    }
}

fn subkey(identity: &PolicyIdentity) -> String {
    format!(r"{POLICIES}\{}", identity.name())
}

fn open(subkey: &str) -> io::Result<Option<Key>> {
    let path = ffi::wide(OsStr::new(subkey));
    let mut handle: HKEY = null_mut();
    // SAFETY: the subkey is NUL terminated and the output slot is a live local;
    // an opened key is owned by the returned Key.
    let opened = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &raw mut handle,
        )
    };
    if opened == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    status(opened)?;
    Ok(Some(Key(handle)))
}

fn raw_values(key: &Key) -> io::Result<Vec<RawValue>> {
    let (mut count, mut longest_name, mut longest_data) = (0u32, 0u32, 0u32);
    // SAFETY: the key is open for reading and every non-null output is a live
    // local; the outputs not asked for are null, which the call allows.
    unsafe {
        status(RegQueryInfoKeyW(
            key.0,
            null_mut(),
            null_mut(),
            null(),
            null_mut(),
            null_mut(),
            null_mut(),
            &raw mut count,
            &raw mut longest_name,
            &raw mut longest_data,
            null_mut(),
            null_mut(),
        ))?;
    }
    let mut values = Vec::with_capacity(count as usize);
    for index in 0..count {
        let mut name = vec![0u16; longest_name as usize + 1];
        let mut data = vec![0u8; longest_data as usize];
        let (mut name_length, mut data_length) = (name.len() as u32, data.len() as u32);
        let mut kind = 0u32;
        // SAFETY: both buffers are live and the lengths passed are their
        // capacities; the call writes at most that much and reports what it
        // wrote.
        unsafe {
            status(RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &raw mut name_length,
                null(),
                &raw mut kind,
                data.as_mut_ptr(),
                &raw mut data_length,
            ))?;
        }
        name.truncate(name_length as usize);
        data.truncate(data_length as usize);
        values.push(RawValue {
            name: String::from_utf16(&name)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            kind,
            data,
        });
    }
    Ok(values)
}

fn text(data: &[u8]) -> io::Result<String> {
    let mut units: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    while units.last() == Some(&0) {
        units.pop();
    }
    String::from_utf16(&units).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn typed(key: &str, raw: RawValue) -> Result<(String, PolicyValue), PolicyError> {
    let value = match (raw.kind, <[u8; 4]>::try_from(raw.data.as_slice())) {
        (REG_SZ | REG_EXPAND_SZ, _) => {
            PolicyValue::Text(text(&raw.data).map_err(|source| PolicyError::Registry {
                key: key.to_owned(),
                source,
            })?)
        },
        (REG_DWORD, Ok(bytes)) => PolicyValue::Number(u32::from_le_bytes(bytes)),
        (kind, _) => {
            return Err(PolicyError::UnsupportedValue {
                key: key.to_owned(),
                name: raw.name,
                kind,
            });
        },
    };
    Ok((raw.name, value))
}

fn read_key(identity: &PolicyIdentity) -> Result<Option<PolicyRead>, PolicyError> {
    let subkey = subkey(identity);
    let key = format!(r"{MACHINE_HIVE}\{subkey}");
    let registry = |source| PolicyError::Registry {
        key: key.clone(),
        source,
    };
    let Some(handle) = open(&subkey).map_err(registry)? else {
        return Ok(None);
    };
    let mut values = BTreeMap::new();
    for raw in raw_values(&handle).map_err(registry)? {
        let (name, value) = typed(&key, raw)?;
        values.insert(name, value);
    }
    Ok(Some(PolicyRead {
        location: PolicyLocation::Registry { key },
        content: PolicyContent::Values(values),
    }))
}

struct Descriptor(*mut c_void);

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: the pointer is null or a descriptor GetSecurityInfo allocated
        // for the caller to release with LocalFree; it is released once.
        unsafe {
            LocalFree(self.0);
        }
    }
}

fn administrator_owned(file: &File) -> io::Result<bool> {
    let mut owner = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: the handle is an open file; the owner and descriptor slots are
    // live locals, and the owner points into the descriptor the Descriptor
    // keeps allocated until both SID checks below have returned.
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
        let trusted = IsWellKnownSid(owner, WinBuiltinAdministratorsSid) != 0
            || IsWellKnownSid(owner, WinLocalSystemSid) != 0;
        drop(descriptor);
        Ok(trusted)
    }
}

fn policy_file(identity: &PolicyIdentity) -> Result<PathBuf, PolicyError> {
    Ok(ffi::program_data()
        .map_err(|source| PolicyError::io("resolve", Path::new("ProgramData"), source))?
        .join(identity.name())
        .join(identity.file_name()))
}

fn read_file(path: PathBuf) -> Result<Option<PolicyRead>, PolicyError> {
    let file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(PolicyError::io("open", &path, source)),
    };
    let regular = file
        .metadata()
        .map_err(|source| PolicyError::io("inspect", &path, source))?
        .is_file();
    let owned = administrator_owned(&file)
        .map_err(|source| PolicyError::io("read the owner of", &path, source))?;
    if !regular || !owned {
        return Err(PolicyError::Untrusted { path });
    }
    let mut bytes = Vec::new();
    file.take(DOCUMENT_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| PolicyError::io("read", &path, source))?;
    if bytes.len() as u64 > DOCUMENT_LIMIT {
        return Err(PolicyError::TooLarge {
            path,
            limit: DOCUMENT_LIMIT,
        });
    }
    Ok(Some(PolicyRead {
        location: PolicyLocation::File { path },
        content: PolicyContent::Document(bytes),
    }))
}

pub(super) fn locations(identity: &PolicyIdentity) -> Result<Vec<PolicyLocation>, PolicyError> {
    Ok(vec![
        PolicyLocation::Registry {
            key: format!(r"{MACHINE_HIVE}\{}", subkey(identity)),
        },
        PolicyLocation::File {
            path: policy_file(identity)?,
        },
    ])
}

pub(super) fn read(identity: &PolicyIdentity) -> Result<Vec<PolicyRead>, PolicyError> {
    let key = read_key(identity)?;
    let file = read_file(policy_file(identity)?)?;
    Ok(key.into_iter().chain(file).collect())
}
