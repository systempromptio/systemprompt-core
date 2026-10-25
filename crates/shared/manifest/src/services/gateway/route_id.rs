//! Model-glob matching and the 0.72 route-id synthesis.
//!
//! [`synthesize_route_id`] is kept only for `config migrate`, which names the
//! ids 0.72 generated for its implicit routes so rules that addressed them can
//! be moved. Declared routes always carry their own id. The hash segment
//! labels are part of that id and never change.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::RouteId;

use systemprompt_identifiers::gateway_hash::fnv1a_segments;

const LEGACY_GLOB_SEGMENT: &str = "model_pattern";

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
pub fn synthesize_route_id(glob: &str, provider: &str) -> RouteId {
    let h = fnv1a_segments(&[
        (LEGACY_GLOB_SEGMENT, glob.as_bytes()),
        ("provider", provider.as_bytes()),
    ]);
    let hash6: String = format!("{h:016x}").chars().take(6).collect();
    RouteId::new(format!("{}-{}", slugify_pattern(glob), hash6))
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
