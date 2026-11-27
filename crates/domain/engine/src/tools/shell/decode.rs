//! Decoding a child's output stream. Most tools print UTF-8 (`PowerShell` is
//! told to); console programs on Windows may print in the console code page
//! instead, so a line that is not valid UTF-8 is decoded with that code page.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const HOLD_LIMIT: usize = 4096;

#[derive(Debug, Default)]
pub(crate) struct Decoder {
    pending: Vec<u8>,
}

impl Decoder {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        if let Some(end) = self.pending.iter().rposition(|byte| *byte == b'\n') {
            let complete: Vec<u8> = self.pending.drain(..=end).collect();
            for line in complete.split_inclusive(|byte| *byte == b'\n') {
                out.push_str(&decode_line(line));
            }
        }
        match std::str::from_utf8(&self.pending) {
            Ok(text) => {
                out.push_str(text);
                self.pending.clear();
            },
            Err(err) if err.error_len().is_none() => {
                let valid = err.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                self.pending.drain(..valid);
            },
            Err(_) if self.pending.len() > HOLD_LIMIT => {
                let rest = std::mem::take(&mut self.pending);
                out.push_str(&decode_line(&rest));
            },
            Err(_) => {},
        }
        out
    }

    pub(crate) fn finish(&mut self) -> String {
        let rest = std::mem::take(&mut self.pending);
        if rest.is_empty() {
            String::new()
        } else {
            decode_line(&rest)
        }
    }
}

fn decode_line(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    code_page_decode(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}

#[cfg(windows)]
use code_page::decode as code_page_decode;

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "console code pages are reachable only through the Win32 API"
)]
mod code_page {
    pub(super) fn decode(bytes: &[u8]) -> Option<String> {
        use windows_sys::Win32::Globalization::{
            CP_OEMCP, MB_ERR_INVALID_CHARS, MultiByteToWideChar,
        };
        use windows_sys::Win32::System::Console::GetConsoleOutputCP;

        let len = i32::try_from(bytes.len()).ok().filter(|len| *len > 0)?;
        // SAFETY: GetConsoleOutputCP takes no arguments and only reads process state.
        let console = unsafe { GetConsoleOutputCP() };
        let code_page = if console == 0 { CP_OEMCP } else { console };
        // SAFETY: the input pointer and length describe `bytes`; a null output buffer
        // asks for the required size.
        let size = unsafe {
            MultiByteToWideChar(
                code_page,
                MB_ERR_INVALID_CHARS,
                bytes.as_ptr(),
                len,
                std::ptr::null_mut(),
                0,
            )
        };
        let capacity = usize::try_from(size).ok().filter(|size| *size > 0)?;
        let mut wide = vec![0u16; capacity];
        // SAFETY: `wide` has exactly `size` elements, as the first call asked for.
        let written = unsafe {
            MultiByteToWideChar(
                code_page,
                MB_ERR_INVALID_CHARS,
                bytes.as_ptr(),
                len,
                wide.as_mut_ptr(),
                size,
            )
        };
        let written = usize::try_from(written)
            .ok()
            .filter(|written| *written > 0)?;
        String::from_utf16(&wide[..written.min(capacity)]).ok()
    }
}

#[cfg(not(windows))]
const fn code_page_decode(_bytes: &[u8]) -> Option<String> {
    None
}
