//! Comment- and string-aware byte scanner for the Postgres DDL we parse.
//!
//! The schema-additivity parser walks SQL one byte at a time looking for
//! `CREATE TABLE` blocks. It must ignore matches that appear inside string
//! literals, dollar-quoted bodies, line comments, and block comments. This
//! module owns the state machine and the cursor helpers shared between
//! [`super::parser`] and any future SQL-shape probing built on top of it.

pub(super) enum LexState {
    Normal,
    SingleQuote,
    DollarQuote(String),
    LineComment,
    BlockComment(u32),
}

pub(super) fn is_top_level(bytes: &[u8], pos: usize) -> bool {
    let mut i = 0usize;
    let mut state = LexState::Normal;
    while i < pos {
        match &mut state {
            LexState::Normal => match bytes[i] {
                b'\'' => {
                    state = LexState::SingleQuote;
                    i += 1;
                },
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    state = LexState::LineComment;
                    i += 2;
                },
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = LexState::BlockComment(1);
                    i += 2;
                },
                b'$' => {
                    if let Some(tag_end) = dollar_tag_end(bytes, i) {
                        let tag = std::str::from_utf8(&bytes[i..=tag_end])
                            .unwrap_or("$$")
                            .to_string();
                        state = LexState::DollarQuote(tag);
                        i = tag_end + 1;
                    } else {
                        i += 1;
                    }
                },
                _ => i += 1,
            },
            LexState::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                    } else {
                        state = LexState::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            },
            LexState::DollarQuote(tag) => {
                let tag_bytes = tag.as_bytes();
                if i + tag_bytes.len() <= bytes.len() && &bytes[i..i + tag_bytes.len()] == tag_bytes
                {
                    i += tag_bytes.len();
                    state = LexState::Normal;
                } else {
                    i += 1;
                }
            },
            LexState::LineComment => {
                if bytes[i] == b'\n' {
                    state = LexState::Normal;
                }
                i += 1;
            },
            LexState::BlockComment(depth) => {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *depth -= 1;
                    i += 2;
                    if *depth == 0 {
                        state = LexState::Normal;
                    }
                } else {
                    i += 1;
                }
            },
        }
    }
    matches!(state, LexState::Normal)
}

fn dollar_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    debug_assert_eq!(bytes[start], b'$');
    let mut j = start + 1;
    while j < bytes.len() {
        let c = bytes[j];
        if c == b'$' {
            return Some(j);
        }
        if !(c.is_ascii_alphanumeric() || c == b'_') {
            return None;
        }
        j += 1;
    }
    None
}

pub(super) fn is_word_boundary_before(bytes: &[u8], pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    let prev = bytes[pos - 1];
    !(prev.is_ascii_alphanumeric() || prev == b'_')
}

pub(super) fn skip_whitespace_and_comments(bytes: &[u8], mut i: usize) -> usize {
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < bytes.len() && bytes[i] == b'-' && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            let mut depth = 1u32;
            while i < bytes.len() && depth > 0 {
                if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        return i;
    }
}

pub(super) fn starts_with_keyword(upper: &str, pos: usize, kw: &str) -> bool {
    if !upper[pos..].starts_with(kw) {
        return false;
    }
    let end = pos + kw.len();
    upper
        .as_bytes()
        .get(end)
        .copied()
        .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == b'_'))
}

/// Read a Postgres identifier (unquoted `[A-Za-z_][A-Za-z0-9_]*` or
/// double-quoted) starting at `i`. Returns `(name, after)` where `name` is the
/// lowercased identifier value.
pub(super) fn read_identifier(bytes: &[u8], i: usize) -> Option<(String, usize)> {
    if bytes.get(i) == Some(&b'"') {
        let mut end = i + 1;
        while end < bytes.len() && bytes[end] != b'"' {
            end += 1;
        }
        if end >= bytes.len() {
            return None;
        }
        let s = std::str::from_utf8(&bytes[i + 1..end]).ok()?.to_string();
        return Some((s, end + 1));
    }
    let start = i;
    let mut end = i;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    if end == start {
        return None;
    }
    let s = std::str::from_utf8(&bytes[start..end])
        .ok()?
        .to_ascii_lowercase();
    Some((s, end))
}

pub(super) fn find_matching_close_paren(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1u32;
    let mut i = from;
    let mut state = LexState::Normal;
    while i < bytes.len() {
        match &mut state {
            LexState::Normal => match bytes[i] {
                b'(' => {
                    depth += 1;
                    i += 1;
                },
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                    i += 1;
                },
                b'\'' => {
                    state = LexState::SingleQuote;
                    i += 1;
                },
                b'-' if bytes.get(i + 1) == Some(&b'-') => {
                    state = LexState::LineComment;
                    i += 2;
                },
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    state = LexState::BlockComment(1);
                    i += 2;
                },
                b'$' => {
                    if let Some(tag_end) = dollar_tag_end(bytes, i) {
                        let tag = std::str::from_utf8(&bytes[i..=tag_end])
                            .unwrap_or("$$")
                            .to_string();
                        state = LexState::DollarQuote(tag);
                        i = tag_end + 1;
                    } else {
                        i += 1;
                    }
                },
                _ => i += 1,
            },
            LexState::SingleQuote => {
                if bytes[i] == b'\'' {
                    if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                    } else {
                        state = LexState::Normal;
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            },
            LexState::DollarQuote(tag) => {
                let tag_bytes = tag.as_bytes();
                if i + tag_bytes.len() <= bytes.len() && &bytes[i..i + tag_bytes.len()] == tag_bytes
                {
                    i += tag_bytes.len();
                    state = LexState::Normal;
                } else {
                    i += 1;
                }
            },
            LexState::LineComment => {
                if bytes[i] == b'\n' {
                    state = LexState::Normal;
                }
                i += 1;
            },
            LexState::BlockComment(depth_b) => {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    *depth_b += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    *depth_b -= 1;
                    i += 2;
                    if *depth_b == 0 {
                        state = LexState::Normal;
                    }
                } else {
                    i += 1;
                }
            },
        }
    }
    None
}
