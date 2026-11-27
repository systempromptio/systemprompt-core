//! Text helpers shared by the file tools: encodings, line endings and char-safe
//! clipping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub(crate) const BOM: &str = "\u{feff}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextFile {
    pub(crate) bom: bool,
    pub(crate) crlf: bool,
    pub(crate) body: String,
}

impl TextFile {
    pub(crate) fn decode(bytes: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(bytes).ok()?;
        let (bom, text) = text
            .strip_prefix(BOM)
            .map_or((false, text), |rest| (true, rest));
        let crlf_count = text.matches("\r\n").count();
        let lf_count = text.matches('\n').count();
        let crlf = crlf_count > 0 && crlf_count * 2 >= lf_count;
        Some(Self {
            bom,
            crlf,
            body: normalize_newlines(text),
        })
    }

    pub(crate) fn plain(body: &str) -> Self {
        Self {
            bom: false,
            crlf: false,
            body: normalize_newlines(body),
        }
    }

    pub(crate) fn encode(&self, body: &str) -> Vec<u8> {
        let body = normalize_newlines(body);
        let mut out = String::with_capacity(body.len() + body.len() / 32 + 4);
        if self.bom {
            out.push_str(BOM);
        }
        if self.crlf {
            out.push_str(&body.replace('\n', "\r\n"));
        } else {
            out.push_str(&body);
        }
        out.into_bytes()
    }
}

pub(crate) fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub(crate) const fn floor_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut at = index;
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

pub(crate) const fn ceil_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut at = index;
    while at < text.len() && !text.is_char_boundary(at) {
        at += 1;
    }
    at
}

pub(crate) fn clip_chars(text: &str, max: usize) -> (&str, bool) {
    match text.char_indices().nth(max) {
        Some((at, _)) => (&text[..at], true),
        None => (text, false),
    }
}

pub(crate) fn title_line(text: &str, max: usize) -> String {
    let first = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    let (clipped, cut) = clip_chars(first, max);
    if cut || text.trim().lines().nth(1).is_some() {
        format!("{clipped}…")
    } else {
        clipped.to_owned()
    }
}

pub(crate) fn looks_binary(sample: &[u8]) -> bool {
    if sample.is_empty() {
        return false;
    }
    if sample.contains(&0) {
        return true;
    }
    let odd = sample
        .iter()
        .filter(|byte| matches!(byte, 0x01..=0x08 | 0x0e..=0x1a | 0x1c..=0x1f))
        .count();
    odd * 10 > sample.len() * 3
}

pub(crate) fn decode_utf16(bytes: &[u8]) -> Option<String> {
    let (little, rest) = match bytes {
        [0xFF, 0xFE, rest @ ..] => (true, rest),
        [0xFE, 0xFF, rest @ ..] => (false, rest),
        _ => return None,
    };
    let units: Vec<u16> = rest
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            if little {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    Some(String::from_utf16_lossy(&units))
}
