//! Minimal `*`-glob matcher for `entity_match` rules.
//!
//! Handles `"*"`, `"prefix*"`, `"*suffix"`, and interior wildcards like
//! `"a*b*c"`. A dedicated glob crate would be overkill for matching catalog ids
//! at ingest time.

pub fn glob_matches(pattern: &str, candidate: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
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
        match candidate[cursor..end].find(part) {
            Some(pos) => cursor += pos + part.len(),
            None => return false,
        }
    }
    true
}
