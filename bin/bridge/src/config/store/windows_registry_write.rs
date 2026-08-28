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
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW,
    RegSetValueExW,
};

use super::ConfigStoreError;
use super::windows_registry::OwnedKey;
use crate::cowork_compat::POLICY_SUBKEY;

pub(crate) fn write_managed_policy_values(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<(), ConfigStoreError> {
    let (hive, hive_label) = if elevated {
        (HKEY_LOCAL_MACHINE, "HKLM")
    } else {
        (HKEY_CURRENT_USER, "HKCU")
    };
    tracing::info!(
        hive = hive_label,
        subkey = POLICY_SUBKEY,
        value_count = entries.len(),
        "writing managed Claude policy via in-process registry FFI"
    );
    let key = create_policy_key(hive, hive_label)?;
    for (name, value) in entries {
        set_string_value(key.0, hive_label, name, value)?;
        tracing::debug!(
            hive = hive_label,
            name = name.as_str(),
            "wrote REG_SZ policy value"
        );
    }
    Ok(())
}
pub(crate) fn delete_managed_policy_values(
    elevated: bool,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    let (hive, hive_label) = if elevated {
        (HKEY_LOCAL_MACHINE, "HKLM")
    } else {
        (HKEY_CURRENT_USER, "HKCU")
    };
    tracing::info!(
        hive = hive_label,
        subkey = POLICY_SUBKEY,
        value_count = names.len(),
        "deleting managed Claude policy values via in-process registry FFI"
    );
    let Some(key) = open_policy_key_for_write(hive, hive_label)? else {
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
            return Err(access_denied(hive_label));
        } else if status != ERROR_FILE_NOT_FOUND {
            return Err(ConfigStoreError::Backend(format!(
                "RegDeleteValueW({name}) failed with status {status}"
            )));
        }
    }
    Ok(removed)
}
fn open_policy_key_for_write(
    hive: HKEY,
    hive_label: &str,
) -> Result<Option<OwnedKey>, ConfigStoreError> {
    let subkey: Vec<u16> = POLICY_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: `hive` is a predefined HKEY, `subkey` is NUL-terminated, and
    // `handle` is a live out-param.
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            subkey.as_ptr(),
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
        Err(access_denied(hive_label))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegOpenKeyExW({POLICY_SUBKEY}) failed with status {status}"
        )))
    }
}
fn create_policy_key(hive: HKEY, hive_label: &str) -> Result<OwnedKey, ConfigStoreError> {
    let subkey: Vec<u16> = POLICY_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: `hive` is a predefined HKEY, `subkey` is NUL-terminated, the null
    // security and class pointers request defaults, and `handle` is a live
    // out-param.
    let status = unsafe {
        RegCreateKeyExW(
            hive,
            subkey.as_ptr(),
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
        Err(access_denied(hive_label))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegCreateKeyExW({POLICY_SUBKEY}) failed with status {status}"
        )))
    }
}
// Why: `SOFTWARE\Policies` is ACL-protected in both hives; a non-elevated
// create/set returns status 5.
fn access_denied(hive_label: &str) -> ConfigStoreError {
    ConfigStoreError::AccessDenied {
        hive: hive_label.to_owned(),
        subkey: POLICY_SUBKEY.to_owned(),
    }
}
fn set_string_value(
    key: HKEY,
    hive_label: &str,
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
        Err(access_denied(hive_label))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegSetValueExW({name}) failed with status {status}"
        )))
    }
}
