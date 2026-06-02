#![cfg(target_os = "windows")]
#![allow(
    unsafe_code,
    reason = "Win32 registry FFI for HKLM/HKCU managed-policy values"
)]

use std::collections::BTreeMap;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE, RegCloseKey, RegCreateKeyExW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};

use super::{ConfigStore, ConfigStoreError, ManagedPolicyRead};

const POLICY_SUBKEY: &str = r"SOFTWARE\Policies\Claude";

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
        // Merge HKLM (machine-wide policy) and HKCU (per-user policy). HKCU is read
        // last so per-user values override machine defaults — matches how the running
        // GUI publishes its live `inferenceGatewayBaseUrl` to HKCU.
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        let mut hives_with_data: Vec<&'static str> = Vec::new();
        for (hive, hive_label) in [(HKEY_LOCAL_MACHINE, "HKLM"), (HKEY_CURRENT_USER, "HKCU")] {
            let Some(handle) = open_policy_key(hive)? else {
                continue;
            };
            let mut hive_had_value = false;
            for key in keys {
                if let Some(v) = read_string_value(handle.0, key)? {
                    values.insert((*key).to_string(), v);
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
}

struct OwnedKey(HKEY);

impl Drop for OwnedKey {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { RegCloseKey(self.0) };
        }
    }
}

fn open_policy_key(hive: HKEY) -> Result<Option<OwnedKey>, ConfigStoreError> {
    let subkey: Vec<u16> = POLICY_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut handle: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            subkey.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut handle,
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
    let probe = unsafe {
        RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            std::ptr::null_mut(),
            &mut byte_len,
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
    let status = unsafe {
        RegQueryValueExW(
            key,
            name_w.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            buffer.as_mut_ptr().cast::<u8>(),
            &mut final_len,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(ConfigStoreError::Backend(format!(
            "RegQueryValueExW read failed with status {status}"
        )));
    }
    let final_wide = (final_len as usize).div_ceil(2);
    let slice = &buffer[..final_wide.min(buffer.len())];
    let trimmed = match slice.iter().position(|c| *c == 0) {
        Some(end) => &slice[..end],
        None => slice,
    };
    Ok(Some(String::from_utf16_lossy(trimmed)))
}

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
    let key = create_policy_key(hive)?;
    for (name, value) in entries {
        set_string_value(key.0, name, value)?;
        tracing::debug!(hive = hive_label, name, "wrote REG_SZ policy value");
    }
    Ok(())
}

fn create_policy_key(hive: HKEY) -> Result<OwnedKey, ConfigStoreError> {
    let subkey: Vec<u16> = POLICY_SUBKEY
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut handle: HKEY = std::ptr::null_mut();
    let status = unsafe {
        RegCreateKeyExW(
            hive,
            subkey.as_ptr(),
            0,
            std::ptr::null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | KEY_WOW64_64KEY,
            std::ptr::null(),
            &mut handle,
            std::ptr::null_mut(),
        )
    };
    if status == ERROR_SUCCESS {
        Ok(OwnedKey(handle))
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegCreateKeyExW({POLICY_SUBKEY}) failed with status {status}"
        )))
    }
}

fn set_string_value(key: HKEY, name: &str, value: &str) -> Result<(), ConfigStoreError> {
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let data_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = u32::try_from(std::mem::size_of_val(data_w.as_slice())).map_err(|_| {
        ConfigStoreError::Backend(format!("value for {name} exceeds the registry size limit"))
    })?;
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
    } else {
        Err(ConfigStoreError::Backend(format!(
            "RegSetValueExW({name}) failed with status {status}"
        )))
    }
}
