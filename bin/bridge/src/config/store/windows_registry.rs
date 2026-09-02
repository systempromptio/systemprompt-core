//! Windows registry (`SOFTWARE\\Policies\\Claude`) policy store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "windows")]
#![allow(
    unsafe_code,
    reason = "Win32 registry FFI for HKLM/HKCU managed-policy values"
)]

use std::collections::BTreeMap;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_SZ, REG_VALUE_TYPE,
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
};

use super::{
    ConfigStore, ConfigStoreError, ManagedPolicyRead, PolicyDocument, PolicyDocumentValue,
    PolicyHive,
};

use crate::cowork_compat::POLICY_SUBKEY;

pub(super) struct WindowsRegistryStore;

impl ConfigStore for WindowsRegistryStore {
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError> {
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            let Some(handle) = open_policy_key(hive)? else {
                continue;
            };
            let value = read_string_value(handle.0, key)?;
            drop(handle);
            if value.is_some() {
                return Ok(value);
            }
        }
        Ok(None)
    }

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError> {
        // Why: HKLM is read last so it wins — Cowork ignores HKCU once the
        // machine key exists, and the probe must see what Cowork sees.
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        let mut hives_with_data: Vec<&'static str> = Vec::new();
        for (hive, hive_label) in [(HKEY_CURRENT_USER, "HKCU"), (HKEY_LOCAL_MACHINE, "HKLM")] {
            let Some(handle) = open_policy_key(hive)? else {
                continue;
            };
            let mut hive_had_value = false;
            for key in keys {
                if let Some(v) = read_string_value(handle.0, key)? {
                    values.insert((*key).to_owned(), v);
                    hive_had_value = true;
                }
            }
            drop(handle);
            if hive_had_value {
                hives_with_data.push(hive_label);
            }
        }
        if values.is_empty() {
            return Ok(ManagedPolicyRead::default());
        }
        let source = match hives_with_data.as_slice() {
            [single] => format!(r"{single}\{POLICY_SUBKEY}"),
            multi => format!("{}\\{POLICY_SUBKEY}", multi.join("+")),
        };
        Ok(ManagedPolicyRead {
            source: Some(source),
            values,
        })
    }

    fn read_policy_document(
        &self,
        hive: PolicyHive,
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        let mut doc = PolicyDocument::new();
        let Some(handle) = open_policy_key(hkey(hive))? else {
            return Ok(doc);
        };
        for key in keys {
            if let Some(v) = read_string_value(handle.0, key)? {
                doc.insert((*key).to_owned(), PolicyDocumentValue::Str(v));
            }
        }
        Ok(doc)
    }

    fn write_policy_values(
        &self,
        hive: PolicyHive,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        super::windows_registry_write::write_policy_values(hive, entries)
    }

    fn delete_policy_values(
        &self,
        hive: PolicyHive,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        super::windows_registry_write::delete_policy_values(hive, names)
    }

    fn delete_policy_key(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        super::windows_registry_write::delete_policy_key(hive)
    }
}

pub(super) const fn hkey(hive: PolicyHive) -> HKEY {
    match hive {
        PolicyHive::Machine => HKEY_LOCAL_MACHINE,
        PolicyHive::User => HKEY_CURRENT_USER,
    }
}

pub(super) struct OwnedKey(pub(super) HKEY);

impl Drop for OwnedKey {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` is a non-null registry key this `OwnedKey` exclusively owns.
            unsafe { RegCloseKey(self.0) };
        }
    }
}

fn open_policy_key(hive: HKEY) -> Result<Option<OwnedKey>, ConfigStoreError> {
    open_key_for_read(hive, POLICY_SUBKEY)
}

pub(crate) fn read_string(
    hive: HKEY,
    subkey: &str,
    name: &str,
) -> Result<Option<String>, ConfigStoreError> {
    let Some(handle) = open_key_for_read(hive, subkey)? else {
        return Ok(None);
    };
    let value = read_string_value(handle.0, name)?;
    drop(handle);
    Ok(value)
}

fn open_key_for_read(hive: HKEY, subkey: &str) -> Result<Option<OwnedKey>, ConfigStoreError> {
    let subkey: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let mut handle: HKEY = std::ptr::null_mut();
    // SAFETY: `hive` is a predefined HKEY, `subkey` is a NUL-terminated UTF-16
    // buffer, and `handle` is a live out-param receiving the opened key.
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            subkey.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &raw mut handle,
        )
    };
    if status == ERROR_SUCCESS {
        Ok(Some(OwnedKey(handle)))
    } else if status == ERROR_FILE_NOT_FOUND {
        Ok(None)
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegOpenKeyExW failed with status {status}"
        )))
    }
}

fn read_string_value(key: HKEY, name: &str) -> Result<Option<String>, ConfigStoreError> {
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut value_type: REG_VALUE_TYPE = 0;
    let mut byte_len: u32 = 0;
    // SAFETY: `key` is a live open key, `name_w` is NUL-terminated, and the null
    // data pointer requests only the size into the live `byte_len` out-param.
    let probe = unsafe {
        RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null_mut(),
            &raw mut value_type,
            std::ptr::null_mut(),
            &raw mut byte_len,
        )
    };
    if probe == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if probe != ERROR_SUCCESS && probe != ERROR_MORE_DATA {
        return Err(ConfigStoreError::Backend(format!(
            "RegQueryValueExW probe failed with status {probe}"
        )));
    }
    if value_type != REG_SZ {
        return Ok(None);
    }
    if byte_len == 0 {
        return Ok(Some(String::new()));
    }
    let wide_len = (byte_len as usize).div_ceil(2);
    let mut buffer: Vec<u16> = vec![0u16; wide_len];
    let mut final_len = byte_len;
    // SAFETY: `key` is live, `name_w` is NUL-terminated, and `buffer` holds
    // `byte_len` bytes matching `final_len`.
    let status = unsafe {
        RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null_mut(),
            &raw mut value_type,
            buffer.as_mut_ptr().cast::<u8>(),
            &raw mut final_len,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(ConfigStoreError::Backend(format!(
            "RegQueryValueExW read failed with status {status}"
        )));
    }
    let final_wide = (final_len as usize).div_ceil(2);
    let slice = &buffer[..final_wide.min(buffer.len())];
    let trimmed = slice
        .iter()
        .position(|c| *c == 0)
        .map_or(slice, |end| &slice[..end]);
    Ok(Some(String::from_utf16_lossy(trimmed)))
}
