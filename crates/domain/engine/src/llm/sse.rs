//! An incremental server-sent-events parser: bytes in, complete frames out.
//!
//! Lines may end in LF, CRLF or a lone CR, and frames may be split anywhere
//! across chunks. Comment lines are dropped; `id` and `retry` fields are
//! ignored because a model stream is never resumed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// One dispatched SSE event: its optional `event:` name and its joined `data:`
/// lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: String,
}

/// Feed raw body chunks with [`SseParser::push`]; call [`SseParser::finish`]
/// once at end of stream.
#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    scanned: usize,
    event: Option<String>,
    data: Option<String>,
    started: bool,
}

impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<SseFrame> {
        self.buffer.extend_from_slice(chunk);
        if !self.started {
            if self.buffer.len() < 3 && b"\xEF\xBB\xBF".starts_with(&self.buffer) {
                return Vec::new();
            }
            if self.buffer.starts_with(b"\xEF\xBB\xBF") {
                self.buffer.drain(..3);
            }
            self.started = true;
        }
        let mut frames = Vec::new();
        let mut start = 0;
        let mut index = self.scanned;
        while index < self.buffer.len() {
            match self.buffer[index] {
                b'\n' => {
                    self.line(start, index, &mut frames);
                    start = index + 1;
                    index = start;
                },
                b'\r' => {
                    let Some(&next) = self.buffer.get(index + 1) else {
                        break;
                    };
                    self.line(start, index, &mut frames);
                    start = if next == b'\n' { index + 2 } else { index + 1 };
                    index = start;
                },
                _ => index += 1,
            }
        }
        self.buffer.drain(..start);
        self.scanned = index - start;
        frames
    }

    pub fn finish(&mut self) -> Vec<SseFrame> {
        let mut frames = Vec::new();
        if !self.buffer.is_empty() {
            let end = if self.buffer.last() == Some(&b'\r') {
                self.buffer.len() - 1
            } else {
                self.buffer.len()
            };
            self.line(0, end, &mut frames);
            self.buffer.clear();
            self.scanned = 0;
        }
        self.dispatch(&mut frames);
        frames
    }

    fn line(&mut self, start: usize, end: usize, frames: &mut Vec<SseFrame>) {
        if start == end {
            self.dispatch(frames);
            return;
        }
        let line = String::from_utf8_lossy(&self.buffer[start..end]).into_owned();
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                },
                None => self.data = Some(value.to_owned()),
            },
            "event" => self.event = Some(value.to_owned()),
            _ => {},
        }
    }

    fn dispatch(&mut self, frames: &mut Vec<SseFrame>) {
        let event = self.event.take();
        if let Some(data) = self.data.take() {
            frames.push(SseFrame {
                event: event.filter(|e| !e.is_empty()),
                data,
            });
        }
    }
}
