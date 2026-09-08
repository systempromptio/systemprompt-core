//! Writing managed-policy values into the Windows registry.
//!
//! Split from `windows_registry.rs`, which keeps the read path. The two differ
//! in more than direction: writing needs an elevated hive handle and has to
//! create the policy key, so it carries its own open/create helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]
#![allow(
    unsafe_code,
    reason = "Win32 registry FFI for HKLM/HKCU managed-policy values"
)]

use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, KEY_WOW64_64KEY, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCreateKeyExW,
    RegDeleteTreeW, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW,
};

use super::windows_registry::{OwnedKey, hkey};
use super::{ConfigStoreError, PolicyDocumentValue, PolicyHive};
use crate::cowork_compat::POLICY_SUBKEY;

pub(super) fn write_values_at(
    hive: PolicyHive,
    subkey: &str,
    entries: &[(String, PolicyDocumentValue)],
) -> Result<(), ConfigStoreError> {
    let hive_label = hive.label();
    tracing::info!(
        hive = hive_label,
        subkey,
        value_count = entries.len(),
        "writing managed policy via in-process registry FFI"
    );
    let key = create_key(hkey(hive), hive_label, subkey)?;
    let mut completed = Vec::new();
    for (name, value) in entries {
        let Some(text) = value.as_str() else {
            return Err(ConfigStoreError::Backend(format!(
                "{name}: Windows policy values are REG_SZ strings"
            )));
        };
        let result = set_string_value(key.0, hive_label, subkey, name, text)
            .and_then(|()| verify_written(hive, subkey, &[(name.clone(), value.clone())]));
        result.map_err(|source| ConfigStoreError::Partial {
            completed: completed.clone(),
            source: Box::new(source),
        })?;
        completed.push(super::verified::PolicyReceipt::new(
            super::PolicyWrite::Written(hive),
            subkey.to_owned(),
            vec![name.clone()],
        ));
    }
    drop(key);
    verify_written(hive, subkey, entries)
}

fn verify_written(
    hive: PolicyHive,
    subkey: &str,
    entries: &[(String, PolicyDocumentValue)],
) -> Result<(), ConfigStoreError> {
    for (name, value) in entries {
        let stored = super::windows_registry::read_string(hkey(hive), subkey, name)?;
        if stored.as_deref() != value.as_str() {
            return Err(ConfigStoreError::VerifyMismatch {
                hive: hive.label().to_owned(),
                subkey: subkey.to_owned(),
                name: name.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn delete_policy_key(hive: PolicyHive) -> Result<bool, ConfigStoreError> {
    let hive_label = hive.label();
    if !super::windows_registry::key_exists(hive, POLICY_SUBKEY)? {
        return Ok(false);
    }
    let parent = open_policy_parent_for_delete(hive)?;
    let subkey: Vec<u16> = "Claude".encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: `hive` is a predefined HKEY and `subkey` is NUL-terminated.
    let status = unsafe { RegDeleteTreeW(parent.0, subkey.as_ptr()) };
    if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
        if super::windows_registry::key_exists(hive, POLICY_SUBKEY)? {
            return Err(ConfigStoreError::VerifyMismatch {
                hive: hive.label().to_owned(),
                subkey: POLICY_SUBKEY.to_owned(),
                name: "<key>".to_owned(),
            });
        }
        Ok(status == ERROR_SUCCESS)
    } else if status == ERROR_ACCESS_DENIED {
        Err(access_denied(hive_label))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegDeleteTreeW({POLICY_SUBKEY}) failed with status {status}"
        )))
    }
}

pub(super) fn delete_values_at(
    hive: PolicyHive,
    subkey: &str,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    let hive_label = hive.label();
    let target_hive = hive;
    let hive = hkey(hive);
    tracing::info!(
        hive = hive_label,
        subkey,
        value_count = names.len(),
        "deleting managed policy values via in-process registry FFI"
    );
    let Some(key) = open_key_for_write(hive, hive_label, subkey)? else {
        return Ok(0);
    };
    let mut removed = 0;
    for name in names {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `key` is a live open key and `name_w` is NUL-terminated.
        let status = unsafe { RegDeleteValueW(key.0, name_w.as_ptr()) };
        if status == ERROR_SUCCESS {
            removed += 1;
        } else if status == ERROR_ACCESS_DENIED {
            return Err(access_denied_at(hive_label, subkey));
        } else if status != ERROR_FILE_NOT_FOUND {
            return Err(ConfigStoreError::Backend(format!(
                "RegDeleteValueW({name}) failed with status {status}"
            )));
        }
    }
    drop(key);
    for name in names {
        if super::windows_registry::read_string(hive, subkey, name)?.is_some() {
            return Err(ConfigStoreError::VerifyMismatch {
                hive: target_hive.label().to_owned(),
                subkey: subkey.to_owned(),
                name: (*name).to_owned(),
            });
        }
    }
    Ok(removed)
}

fn open_policy_parent_for_delete(hive: PolicyHive) -> Result<OwnedKey, ConfigStoreError> {
    let mut handle: HKEY = std::ptr::null_mut();
    let parent: Vec<u16> = r"SOFTWARE\Policies"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: the hive is predefined, the parent subkey is terminated, and handle
    // is a live out-param.
    let status = unsafe {
        RegOpenKeyExW(
            hkey(hive),
            parent.as_ptr(),
            0,
            windows_sys::Win32::System::Registry::KEY_ALL_ACCESS | KEY_WOW64_64KEY,
            &raw mut handle,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(ConfigStoreError::Backend(format!(
            "{}: opening registry root for deletion failed ({status})",
            hive.label()
        )));
    }
    Ok(OwnedKey(handle))
}

fn open_key_for_write(
    hive: HKEY,
    hive_label: &str,
    subkey: &str,
) -> Result<Option<OwnedKey>, ConfigStoreError> {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: `hive` is a predefined HKEY, `subkey_w` is NUL-terminated, and
    // `handle` is a live out-param.
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            subkey_w.as_ptr(),
            0,
            KEY_WRITE | KEY_WOW64_64KEY,
            &raw mut handle,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(Some(OwnedKey(handle)))
    } else if status == ERROR_FILE_NOT_FOUND {
        Ok(None)
    } else if status == ERROR_ACCESS_DENIED {
        Err(access_denied_at(hive_label, subkey))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegOpenKeyExW({subkey}) failed with status {status}"
        )))
    }
}
fn create_key(hive: HKEY, hive_label: &str, subkey: &str) -> Result<OwnedKey, ConfigStoreError> {
    let subkey_w: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: `hive` is a predefined HKEY, `subkey` is NUL-terminated, the null
    // security and class pointers request defaults, and `handle` is a live
    // out-param.
    let status = unsafe {
        RegCreateKeyExW(
            hive,
            subkey_w.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_WOW64_64KEY,
            std::ptr::null(),
            &raw mut handle,
            std::ptr::null_mut(),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(OwnedKey(handle))
    } else if status == ERROR_ACCESS_DENIED {
        Err(access_denied_at(hive_label, subkey))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegCreateKeyExW({subkey}) failed with status {status}"
        )))
    }
}
fn access_denied(hive_label: &str) -> ConfigStoreError {
    access_denied_at(hive_label, POLICY_SUBKEY)
}

fn access_denied_at(hive_label: &str, subkey: &str) -> ConfigStoreError {
    ConfigStoreError::AccessDenied {
        hive: hive_label.to_owned(),
        subkey: subkey.to_owned(),
    }
}
fn set_string_value(
    key: HKEY,
    hive_label: &str,
    subkey: &str,
    name: &str,
    value: &str,
) -> Result<(), ConfigStoreError> {
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let data_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = u32::try_from(size_of_val(data_w.as_slice())).map_err(|e| {
        ConfigStoreError::Backend(format!(
            "value for {name} exceeds the registry size limit: {e}"
        ))
    })?;
    // SAFETY: `key` is a live open key, `name_w` is NUL-terminated, and `data_w`
    // holds `byte_len` bytes of REG_SZ payload.
    let status = unsafe {
        RegSetValueExW(
            key,
            name_w.as_ptr(),
            0,
            REG_SZ,
            data_w.as_ptr().cast::<u8>(),
            byte_len,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(())
    } else if status == ERROR_ACCESS_DENIED {
        Err(access_denied_at(hive_label, subkey))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegSetValueExW({name}) failed with status {status}"
        )))
    }
}
