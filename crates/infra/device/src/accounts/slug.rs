//! Instance keys: a slug of the account's label, unique among the accounts of
//! this install.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

const SLUG_MAX_LEN: usize = 32;

const FALLBACK: &str = "instance";

pub(crate) fn slug(label: &str) -> String {
    let mut out = String::with_capacity(label.len().min(SLUG_MAX_LEN));
    let mut after_dash = true;
    for c in label.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            after_dash = false;
        } else if !after_dash {
            out.push('-');
            after_dash = true;
        }
    }
    out.truncate(SLUG_MAX_LEN);
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return FALLBACK.to_owned();
    }
    out
}

pub(crate) fn unique_slug(label: &str, taken: &[&str]) -> String {
    let base = slug(label);
    if !taken.contains(&base.as_str()) {
        return base;
    }
    let mut suffix = 2u32;
    loop {
        let candidate = format!("{base}-{suffix}");
        if !taken.contains(&candidate.as_str()) {
            return candidate;
        }
        suffix = suffix.saturating_add(1);
    }
}
