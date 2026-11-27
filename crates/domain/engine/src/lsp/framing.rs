//! `Content-Length` framing of JSON-RPC messages on a byte stream.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const HEADER_END: &[u8] = b"\r\n\r\n";
const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Why a byte stream could not be split into frames. The stream is unusable
/// afterwards.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("frame header is not ASCII")]
    NonAsciiHeader,
    #[error("frame header has no Content-Length")]
    MissingLength,
    #[error("frame Content-Length {value:?} is not a number")]
    InvalidLength { value: String },
    #[error("frame of {0} bytes exceeds the limit")]
    TooLarge(usize),
    #[error("frame header is longer than 8 KiB")]
    HeaderTooLong,
}

pub fn encode_frame(body: &[u8]) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut frame = Vec::with_capacity(header.len() + body.len());
    frame.extend_from_slice(header.as_bytes());
    frame.extend_from_slice(body);
    frame
}

/// Accumulates bytes as they arrive and yields whole message bodies.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
    body_length: Option<usize>,
}

impl FrameDecoder {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        let length = if let Some(length) = self.body_length {
            length
        } else {
            let Some(end) = find(&self.buffer, HEADER_END) else {
                if self.buffer.len() > MAX_HEADER_BYTES {
                    return Err(FrameError::HeaderTooLong);
                }
                return Ok(None);
            };
            if end > MAX_HEADER_BYTES {
                return Err(FrameError::HeaderTooLong);
            }
            let length = parse_header(&self.buffer[..end])?;
            self.buffer.drain(..end + HEADER_END.len());
            self.body_length = Some(length);
            length
        };
        if self.buffer.len() < length {
            return Ok(None);
        }
        let rest = self.buffer.split_off(length);
        let body = std::mem::replace(&mut self.buffer, rest);
        self.body_length = None;
        Ok(Some(body))
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_header(raw: &[u8]) -> Result<usize, FrameError> {
    if !raw.is_ascii() {
        return Err(FrameError::NonAsciiHeader);
    }
    let Ok(text) = std::str::from_utf8(raw) else {
        return Err(FrameError::NonAsciiHeader);
    };
    let mut length = None;
    for line in text.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            let value = value.trim();
            let Ok(parsed) = value.parse::<usize>() else {
                return Err(FrameError::InvalidLength {
                    value: value.to_owned(),
                });
            };
            length = Some(parsed);
        }
    }
    let length = length.ok_or(FrameError::MissingLength)?;
    if length > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(length));
    }
    Ok(length)
}
