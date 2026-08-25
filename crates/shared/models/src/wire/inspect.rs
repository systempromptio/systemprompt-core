//! Inspection surface for outbound wire bodies.
//!
//! The gateway forwards some requests as the caller's own bytes rather than
//! rebuilding them from
//! [`CanonicalRequest`](super::canonical::CanonicalRequest). Governance still
//! reasons about the canonical form, and that form is lossy by construction:
//! the inbound parser drops any content block whose `type` it does not model,
//! images carry no text, and `structuredContent` / `_meta` have no
//! canonical home. Anything in one of those places would reach the provider
//! without a scanner ever seeing it.
//!
//! [`string_leaves`] closes that gap by reading the bytes that are actually
//! going upstream and collecting every string in them, whatever shape the JSON
//! takes. Attaching the result to the canonical request makes the scan surface
//! a superset of the forwarded surface, so "inspected" and "sent" cannot
//! diverge.
//!
//! The response direction has the same gap and the same remedy, against the
//! bytes the *client* receives rather than the bytes the provider sent:
//! [`string_leaves`] for a buffered reply, [`sse_string_leaves`] for a
//! streamed one, whose bytes are concatenated SSE frames and not a JSON
//! document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

// JSON: protocol boundary — the walk is over an arbitrary provider wire body.
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceLeaf {
    pub path: String,
    pub value: String,
}

/// `truncated` means a budget stopped the walk, so the surface is a subset of
/// the body and a scanner reading it may miss content that was still sent.
/// Callers record that; it is never a silent success.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForwardedSurface {
    leaves: Vec<SurfaceLeaf>,
    truncated: bool,
}

impl ForwardedSurface {
    #[must_use]
    pub fn leaves(&self) -> &[SurfaceLeaf] {
        &self.leaves
    }

    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.leaves.len()
    }

    #[must_use]
    pub fn joined(&self) -> String {
        let mut out = String::new();
        for leaf in &self.leaves {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&leaf.value);
        }
        out
    }
}

/// A forwarded body is caller-controlled, so every dimension an attacker could
/// grow without bound has a ceiling here.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceBudget {
    pub depth: usize,
    pub leaves: usize,
    pub total_bytes: usize,
    pub leaf_bytes: usize,
}

impl Default for SurfaceBudget {
    fn default() -> Self {
        Self {
            // Why: legitimate provider content nests a handful of levels; this
            // is far above that and far below what would exhaust the walk.
            depth: 64,
            leaves: 50_000,
            total_bytes: 2 * 1024 * 1024,
            leaf_bytes: 64 * 1024,
        }
    }
}

#[must_use]
pub fn string_leaves(body: &[u8], budget: SurfaceBudget) -> ForwardedSurface {
    let Ok(root) = serde_json::from_slice::<Value>(body) else {
        return ForwardedSurface::default();
    };
    let mut surface = ForwardedSurface::default();
    let mut total: usize = 0;
    walk(&mut surface, &mut total, &root, budget);
    surface
}

#[must_use]
pub fn sse_string_leaves(frames: &[u8], budget: SurfaceBudget) -> ForwardedSurface {
    let mut surface = ForwardedSurface::default();
    let mut total: usize = 0;
    let mut rest = frames;
    while !rest.is_empty() {
        let (frame, next) = rest.split_at(super::sse::frame_end(rest).unwrap_or(rest.len()));
        if let Some(payload) = data_payload(frame)
            && let Ok(root) = serde_json::from_slice::<Value>(&payload)
            && !walk(&mut surface, &mut total, &root, budget)
        {
            return surface;
        }
        rest = next;
    }
    surface
}

fn data_payload(frame: &[u8]) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::new();
    let mut found = false;
    for line in frame.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(value) = line.strip_prefix(b"data:") else {
            continue;
        };
        found = true;
        out.extend_from_slice(value.strip_prefix(b" ").unwrap_or(value));
    }
    found.then_some(out)
}

// Why: `false` means a budget is exhausted and no further root may be walked,
// which is what lets one budget span every frame of a stream.
fn walk(
    surface: &mut ForwardedSurface,
    total: &mut usize,
    root: &Value,
    budget: SurfaceBudget,
) -> bool {
    let mut stack: Vec<(&Value, String, usize)> = vec![(root, String::from("$"), 0)];

    while let Some((value, path, depth)) = stack.pop() {
        if depth > budget.depth {
            surface.truncated = true;
            continue;
        }
        match value {
            Value::String(s) => {
                if !push_leaf(surface, total, &budget, &path, s) {
                    return false;
                }
            },
            Value::Array(items) => {
                // Why: pushed in reverse so popping yields document order.
                for (index, item) in items.iter().enumerate().rev() {
                    stack.push((item, format!("{path}[{index}]"), depth + 1));
                }
            },
            Value::Object(map) => {
                for (key, item) in map.iter().rev() {
                    // Why: a credential used as an object key is pathological
                    // but costs nothing to cover, and skipping it would be a
                    // blind spot chosen on the basis of shape.
                    if !push_leaf(surface, total, &budget, &format!("{path}.{key}.$key"), key) {
                        return false;
                    }
                    stack.push((item, format!("{path}.{key}"), depth + 1));
                }
            },
            Value::Null | Value::Bool(_) | Value::Number(_) => {},
        }
    }
    true
}

// Why: `false` means a budget is exhausted and the whole walk must stop, not
// that this one leaf was skipped; `surface.truncated` is set in that case.
fn push_leaf(
    surface: &mut ForwardedSurface,
    total: &mut usize,
    budget: &SurfaceBudget,
    path: &str,
    value: &str,
) -> bool {
    if value.is_empty() {
        return true;
    }
    if surface.leaves.len() >= budget.leaves || *total >= budget.total_bytes {
        surface.truncated = true;
        return false;
    }
    let (value, clipped) = clip(value, budget.leaf_bytes);
    if clipped {
        surface.truncated = true;
    }
    *total += value.len();
    surface.leaves.push(SurfaceLeaf {
        path: path.to_owned(),
        value,
    });
    true
}

// Why: both ends are kept because a credential in a large blob sits at one end
// far more often than in the middle, and keeping both costs the same as one.
fn clip(value: &str, limit: usize) -> (String, bool) {
    if value.len() <= limit {
        return (value.to_owned(), false);
    }
    let half = limit / 2;
    let head_end = crate::text::floor_char_boundary(value, half);
    let tail_start = ceil_boundary(value, value.len() - half);
    let mut out = String::with_capacity(limit + 1);
    out.push_str(&value[..head_end]);
    out.push('\n');
    out.push_str(&value[tail_start..]);
    (out, true)
}

const fn ceil_boundary(s: &str, mut index: usize) -> usize {
    while index < s.len() && !s.is_char_boundary(index) {
        index += 1;
    }
    index
}
