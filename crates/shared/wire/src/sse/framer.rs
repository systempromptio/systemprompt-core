//! Incremental Server-Sent Events framer.
//!
//! [`SseFramer`] turns upstream bytes, cut at whatever boundaries the network
//! produced, into complete event frames. It follows the event-stream grammar:
//! a line ends at CRLF, LF or a lone CR; a blank line dispatches the frame;
//! a line starting with `:` is a comment; `data` lines are joined with `\n`;
//! the single space after a field's colon is optional. Payload bytes are
//! decoded only once a frame is complete, so a UTF-8 sequence split across
//! chunks decodes intact.
//!
//! [`SseFramer::push`] yields decoded data frames; [`SseFramer::push_raw`]
//! yields the exact bytes of every terminated frame, data-less ones included,
//! so a passthrough can forward whole frames. Use one mode per framer: the
//! raw frames plus [`SseFramer::remainder`] always concatenate to the input.
//!
//! Every byte is scanned once: the framer resumes where the previous chunk
//! stopped, and a frame that grows past its byte cap without a terminator is
//! rejected with [`SseFrameTooLarge`] instead of being buffered further.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

pub const MAX_SSE_FRAME_BYTES: usize = 16 * 1024 * 1024;

/// A frame that grew past the framer's byte cap before its terminator arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("upstream SSE frame exceeds {limit} bytes without a terminator")]
pub struct SseFrameTooLarge {
    pub limit: usize,
}

/// One dispatched event frame: its `data` lines joined with `\n`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub data: String,
}

struct Dispatched {
    data: Option<String>,
    raw: Vec<u8>,
}

/// Incremental framer over an upstream SSE byte stream.
#[derive(Debug)]
pub struct SseFramer {
    buf: Vec<u8>,
    scan: usize,
    line_start: usize,
    skip_lf: bool,
    data: Vec<u8>,
    has_data: bool,
    max_frame_bytes: usize,
    failed: bool,
}

impl Default for SseFramer {
    fn default() -> Self {
        Self::with_max_frame_bytes(MAX_SSE_FRAME_BYTES)
    }
}

impl SseFramer {
    #[must_use]
    pub const fn with_max_frame_bytes(max_frame_bytes: usize) -> Self {
        Self {
            buf: Vec::new(),
            scan: 0,
            line_start: 0,
            skip_lf: false,
            data: Vec::new(),
            has_data: false,
            max_frame_bytes,
            failed: false,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<Result<SseFrame, SseFrameTooLarge>> {
        self.feed(chunk, false)
            .into_iter()
            .filter_map(|out| match out {
                Ok(Dispatched { data, .. }) => data.map(|data| Ok(SseFrame { data })),
                Err(e) => Some(Err(e)),
            })
            .collect()
    }

    pub fn push_raw(&mut self, chunk: &[u8]) -> Vec<Result<Vec<u8>, SseFrameTooLarge>> {
        self.feed(chunk, true)
            .into_iter()
            .map(|out| out.map(|d| d.raw))
            .collect()
    }

    fn feed(&mut self, chunk: &[u8], keep_raw: bool) -> Vec<Result<Dispatched, SseFrameTooLarge>> {
        let mut out = Vec::new();
        if self.failed {
            return out;
        }
        self.buf.extend_from_slice(chunk);
        if self.skip_lf && self.buf.len() > self.line_start {
            self.skip_lf = false;
            if self.buf[self.line_start] == b'\n' {
                self.line_start += 1;
                self.scan = self.line_start;
            }
        }
        let mut frame_start = 0;
        while let Some(offset) = self.buf[self.scan..]
            .iter()
            .position(|b| matches!(b, b'\n' | b'\r'))
        {
            let end = self.scan + offset;
            let terminator = match (self.buf[end], self.buf.get(end + 1)) {
                (b'\r', Some(b'\n')) => 2,
                (b'\r', None) => {
                    self.skip_lf = true;
                    1
                },
                _ => 1,
            };
            let line_start = self.line_start;
            self.line_start = end + terminator;
            self.scan = self.line_start;
            if end == line_start {
                let raw = if keep_raw {
                    self.buf[frame_start..self.line_start].to_vec()
                } else {
                    Vec::new()
                };
                out.push(Ok(Dispatched {
                    data: self.dispatch(),
                    raw,
                }));
                frame_start = self.line_start;
            } else {
                self.absorb_line(line_start, end);
                if let Err(e) = self.check_size(self.line_start - frame_start) {
                    out.push(Err(e));
                    return out;
                }
            }
        }
        self.scan = self.buf.len();
        if let Err(e) = self.check_size(self.buf.len() - frame_start) {
            out.push(Err(e));
            return out;
        }
        self.buf.drain(..frame_start);
        self.scan -= frame_start;
        self.line_start -= frame_start;
        out
    }

    #[must_use]
    pub fn remainder(&self) -> &[u8] {
        &self.buf
    }

    fn check_size(&mut self, frame_len: usize) -> Result<(), SseFrameTooLarge> {
        if frame_len <= self.max_frame_bytes {
            return Ok(());
        }
        self.failed = true;
        self.buf = Vec::new();
        self.data = Vec::new();
        self.scan = 0;
        self.line_start = 0;
        Err(SseFrameTooLarge {
            limit: self.max_frame_bytes,
        })
    }

    fn absorb_line(&mut self, start: usize, end: usize) {
        let line = &self.buf[start..end];
        if line[0] == b':' {
            return;
        }
        let (field, value) =
            line.iter()
                .position(|b| *b == b':')
                .map_or((line, &[][..]), |colon| {
                    let value = &line[colon + 1..];
                    (&line[..colon], value.strip_prefix(b" ").unwrap_or(value))
                });
        if field != b"data" {
            return;
        }
        if self.has_data {
            self.data.push(b'\n');
        }
        self.data.extend_from_slice(value);
        self.has_data = true;
    }

    fn dispatch(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.has_data) {
            return None;
        }
        let data = String::from_utf8_lossy(&self.data).into_owned();
        self.data.clear();
        Some(data)
    }
}
