//! Model-pattern matching and stable route-id synthesis.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RouteId;

use crate::gateway_hash::fnv1a_segments;

#[must_use]
pub fn slugify_pattern(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut last_dash = false;
    for ch in pattern.chars() {
        if ch == '*' {
            out.push_str("star");
            last_dash = false;
        } else if ch.is_ascii_alphanumeric() {
            for lc in ch.to_lowercase() {
                out.push(lc);
            }
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    if out.is_empty() {
        out.push_str("route");
    }
    out
}

#[must_use]
pub fn synthesize_route_id(model_pattern: &str, provider: &str) -> RouteId {
    let h = fnv1a_segments(&[
        ("model_pattern", model_pattern.as_bytes()),
        ("provider", provider.as_bytes()),
    ]);
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    RouteId::new(format!("{}-{}", slugify_pattern(model_pattern), hash6))
}

pub(crate) fn match_pattern(pattern: &str, model: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return model.ends_with(suffix);
    }
    pattern == model
}
