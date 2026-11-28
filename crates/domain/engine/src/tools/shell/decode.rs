//! Decoding a child's output stream. Most tools print UTF-8 (`PowerShell` is
//! told to), so a line that is valid UTF-8 is taken as it is; any other line is
//! read the way the platform reads text that is not UTF-8.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::platform;

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
    std::str::from_utf8(bytes).map_or_else(|_| platform::decode(bytes), str::to_owned)
}
