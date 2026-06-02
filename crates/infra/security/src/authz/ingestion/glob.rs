//! Minimal `*`-glob matcher for `entity_match` rules.
//!
//! Handles `"*"`, `"prefix*"`, `"*suffix"`, and interior wildcards like
//! `"a*b*c"`. A dedicated glob crate would be overkill for matching catalog ids
//! at ingest time.
//!
//! Matching operates on raw bytes (`&[u8]`), not `char` boundaries: a candidate
//! may carry multibyte UTF-8 and slicing it on a non-boundary byte would panic.
//! For ASCII ids — every id this matcher actually sees — byte and char matching
//! are identical.

pub fn glob_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.as_bytes();
    let candidate = candidate.as_bytes();
    let parts: Vec<&[u8]> = pattern.split(|&b| b == b'*').collect();
    if parts.len() == 1 {
        return pattern == candidate;
    }
    let first = parts[0];
    let last = parts[parts.len() - 1];
    if !candidate.starts_with(first) || !candidate.ends_with(last) {
        return false;
    }
    if first.len() + last.len() > candidate.len() {
        return false;
    }
    let mut cursor = first.len();
    let end = candidate.len() - last.len();
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        match find_subslice(&candidate[cursor..end], part) {
            Some(pos) => cursor += pos + part.len(),
            None => return false,
        }
    }
    true
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
