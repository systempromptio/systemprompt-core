//! `registry.txt` for the diagnostics bundle: both policy hives, value by
//! value, with the key's owner, DACL and last write, plus the token this
//! process runs under. Secrets are shown as a fingerprint and a length.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(target_os = "windows")]
#[must_use]
pub(crate) fn render() -> String {
    let mut out: Vec<String> = Vec::new();
    out.push(format!(
        "process: {}",
        crate::windows_acl::elevation_summary().unwrap_or_else(|e| format!("<{e}>"))
    ));
    out.push(String::new());
    let subkeys = [
        crate::cowork_compat::POLICY_SUBKEY.to_owned(),
        crate::config::store::bridge_policy_subkey(),
    ];
    for (hive, label) in windows::HIVES {
        for subkey in &subkeys {
            out.push(format!("{label}\\{subkey}:"));
            match windows::describe_key(hive, subkey) {
                Ok(Some(lines)) => out.extend(lines.into_iter().map(|l| format!("  {l}"))),
                Ok(None) => out.push("  <absent>".to_owned()),
                Err(e) => out.push(format!("  <{e}>")),
            }
            out.push(String::new());
        }
    }
    out.push(format!(
        "webview2 runtime: {}",
        crate::gui::webview2::runtime_version().unwrap_or_else(|| "<absent>".to_owned())
    ));
    out.join("\n") + "\n"
}

#[cfg(not(target_os = "windows"))]
#[must_use]
pub(crate) fn render() -> String {
    "<no registry on this platform>\n".to_owned()
}

#[cfg(target_os = "windows")]
mod windows {
    #![allow(unsafe_code, reason = "Win32 registry enumeration FFI")]

    use std::io;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{
        ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, FILETIME,
    };
    use windows_sys::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY, REG_SZ,
        RegCloseKey, RegEnumValueW, RegGetKeySecurity, RegOpenKeyExW, RegQueryInfoKeyW,
    };

    pub(super) const HIVES: [(HKEY, &str); 2] =
        [(HKEY_LOCAL_MACHINE, "HKLM"), (HKEY_CURRENT_USER, "HKCU")];

    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn open(hive: HKEY, subkey: &str) -> io::Result<Option<Key>> {
        let wide: Vec<u16> = subkey.encode_utf16().chain(Some(0)).collect();
        let mut handle: HKEY = null_mut();
        // SAFETY: `hive` is a predefined key, `wide` is NUL terminated and
        // `handle` is a live out-param.
        let status = unsafe {
            RegOpenKeyExW(
                hive,
                wide.as_ptr(),
                0,
                KEY_READ | KEY_WOW64_64KEY,
                &raw mut handle,
            )
        };
        match status {
            ERROR_SUCCESS => Ok(Some(Key(handle))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            other => Err(io::Error::from_raw_os_error(other.cast_signed())),
        }
    }

    pub(super) fn describe_key(hive: HKEY, subkey: &str) -> io::Result<Option<Vec<String>>> {
        let Some(key) = open(hive, subkey)? else {
            return Ok(None);
        };
        let mut lines = Vec::new();
        lines.push(format!("security: {}", security(&key)?));
        let (count, max_name, max_data, written) = info(&key)?;
        lines.push(format!("last write: {written} ({count} values)"));
        for index in 0..count {
            lines.push(value(&key, index, max_name, max_data)?);
        }
        Ok(Some(lines))
    }

    fn security(key: &Key) -> io::Result<String> {
        let info = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let mut size = 0u32;
        // SAFETY: a null descriptor with a zero size asks for the required size.
        unsafe { RegGetKeySecurity(key.0, info, null_mut(), &raw mut size) };
        let mut buffer = vec![0u8; size as usize];
        // SAFETY: `buffer` holds `size` bytes and `size` is a live out-param.
        let status =
            unsafe { RegGetKeySecurity(key.0, info, buffer.as_mut_ptr().cast(), &raw mut size) };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
        let mut text = null_mut();
        let mut length = 0u32;
        // SAFETY: `buffer` is a self-relative descriptor; the string Windows
        // allocates is freed by `Descriptor`.
        unsafe {
            if ConvertSecurityDescriptorToStringSecurityDescriptorW(
                buffer.as_ptr().cast_mut().cast(),
                SDDL_REVISION_1,
                info,
                &raw mut text,
                &raw mut length,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let allocation = crate::windows_acl::Descriptor(text.cast());
            let mut n = 0;
            while *text.add(n) != 0 {
                n += 1;
            }
            let sddl = String::from_utf16(std::slice::from_raw_parts(text, n));
            drop(allocation);
            sddl.map_err(io::Error::other)
        }
    }

    fn info(key: &Key) -> io::Result<(u32, u32, u32, String)> {
        let (mut values, mut max_name, mut max_data) = (0u32, 0u32, 0u32);
        let mut written = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        // SAFETY: every out-param is a live local; unused ones are null.
        let status = unsafe {
            RegQueryInfoKeyW(
                key.0,
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                null_mut(),
                &raw mut values,
                &raw mut max_name,
                &raw mut max_data,
                null_mut(),
                &raw mut written,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
        let ticks = (u64::from(written.dwHighDateTime) << 32) | u64::from(written.dwLowDateTime);
        let unix = i64::try_from(ticks / 10_000_000)
            .map(|secs| secs - 11_644_473_600)
            .unwrap_or_default();
        let stamp = chrono::DateTime::from_timestamp(unix, 0)
            .map_or_else(|| unix.to_string(), |t| t.to_rfc3339());
        Ok((values, max_name, max_data, stamp))
    }

    fn value(key: &Key, index: u32, max_name: u32, max_data: u32) -> io::Result<String> {
        let mut name = vec![0u16; max_name as usize + 1];
        let mut name_len = name.len() as u32;
        let mut data = vec![0u8; max_data as usize + 2];
        let mut data_len = data.len() as u32;
        let mut kind = 0u32;
        // SAFETY: both buffers are sized from RegQueryInfoKeyW and every
        // length is a live out-param.
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &raw mut name_len,
                null_mut(),
                &raw mut kind,
                data.as_mut_ptr(),
                &raw mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            return Ok("<no more values>".to_owned());
        }
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status.cast_signed()));
        }
        let name = String::from_utf16_lossy(&name[..name_len as usize]);
        if kind != REG_SZ {
            return Ok(format!("{name} = <type {kind}, {data_len} bytes>"));
        }
        let units: Vec<u16> = data[..data_len as usize]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        let text = String::from_utf16_lossy(&units);
        Ok(format!("{name} = {}", shown(&name, &text)))
    }

    // Why: the bearer inside managedMcpServers and the gateway API key are
    // credentials; a fingerprint is enough to tell two bridges apart.
    fn shown(name: &str, text: &str) -> String {
        if crate::config::redaction::is_sensitive_key(name) || name == "managedMcpServers" {
            format!(
                "<{} chars, sha256 {}>",
                text.len(),
                &crate::hash::sha256_hex(text.as_bytes())[..8]
            )
        } else {
            text.to_owned()
        }
    }
}
