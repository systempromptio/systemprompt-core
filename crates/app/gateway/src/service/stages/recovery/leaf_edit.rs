//! In-place edits of string leaves inside the provider-bound JSON body.
//!
//! Every string leaf is addressed twice: by the dotted inspection path a
//! scanner reports and by the JSON pointer used to edit it. A leaf is only
//! editable when nothing on its path is provider-signed or protocol-bearing;
//! an edit set that would touch one of those, or whose edit no longer matches
//! what was scanned, is abandoned as a whole so a half-edited body is never
//! forwarded. Secret repair and safety redaction both edit through here, and
//! both keep the canonical request in step with the body they rewrite.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::ops::Range;

use bytes::Bytes;
// JSON: provider wire payloads contain arbitrary client-defined tool arguments
// and metadata.
use serde_json::{Map, Value};
use systemprompt_wire::canonical::CanonicalRequest;
use systemprompt_wire::inspect;

use super::canonical::replace_canonical;
use super::inspection_budget;
use crate::protocol::outbound::PreparedBody;

pub(super) enum LeafEdit {
    Spans(Vec<(Range<usize>, String)>),
    Replace(String),
}

pub(super) fn apply_leaf_edits(
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
    edits: Vec<(String, LeafEdit)>,
) -> Option<()> {
    let surface = inspect::string_leaves(&body.bytes, inspection_budget());
    if surface.truncated() || edits.is_empty() {
        return None;
    }
    let mut root: Value = serde_json::from_slice(&body.bytes).ok()?;
    let locations = locations(&root)?;
    let mut replacements = Vec::new();
    for (path, edit) in edits {
        let leaf = surface.leaves().iter().find(|leaf| leaf.path == path)?;
        let location = locations.get(&path)?;
        if !location.editable {
            return None;
        }
        let slot = root.pointer_mut(&location.pointer)?;
        let value = slot.as_str()?;
        if value != leaf.value {
            return None;
        }
        let edited = match edit {
            LeafEdit::Spans(spans) => {
                let edited = mark_spans(value, &spans)?;
                for (span, marker) in spans {
                    replacements.push((value.get(span)?.to_owned(), marker));
                }
                edited
            },
            LeafEdit::Replace(text) => text,
        };
        if location.json_string && serde_json::from_str::<Value>(&edited).is_err() {
            return None;
        }
        replacements.push((value.to_owned(), edited.clone()));
        *slot = Value::String(edited);
    }
    let bytes = Bytes::from(serde_json::to_vec(&root).ok()?);
    let surface = inspect::string_leaves(&bytes, inspection_budget());
    if surface.truncated() {
        return None;
    }
    replacements.sort_by_key(|(old, _)| std::cmp::Reverse(old.len()));
    let mut edited = request.clone();
    replace_canonical(&mut edited, &replacements);
    edited.forwarded_surface = surface;
    *request = edited;
    body.bytes = bytes;
    Some(())
}

fn mark_spans(value: &str, spans: &[(Range<usize>, String)]) -> Option<String> {
    let mut ordered: Vec<_> = spans.iter().collect();
    ordered.sort_unstable_by_key(|(span, _)| (span.start, span.end));
    let mut out = String::new();
    let mut cursor = 0;
    for (span, marker) in ordered {
        if span.start >= span.end || value.get(span.clone()).is_none() {
            return None;
        }
        if span.end <= cursor {
            continue;
        }
        if span.start >= cursor {
            out.push_str(value.get(cursor..span.start)?);
            out.push_str(marker);
        }
        cursor = span.end;
    }
    out.push_str(value.get(cursor..)?);
    Some(out)
}

struct Location {
    pointer: String,
    editable: bool,
    json_string: bool,
}

impl Location {
    const fn opaque() -> Self {
        Self {
            pointer: String::new(),
            editable: false,
            json_string: false,
        }
    }
}

struct Frame<'a> {
    // JSON: gateway recovery — walks the provider-bound wire body to redact located secrets.
    value: &'a Value,
    path: String,
    pointer: String,
    editable: bool,
    json_string: bool,
}

// JSON: gateway recovery — walks the provider-bound wire body to redact located
// secrets.
fn locations(root: &Value) -> Option<HashMap<String, Location>> {
    let mut out = HashMap::new();
    let mut stack = vec![Frame {
        value: root,
        path: "$".to_owned(),
        pointer: String::new(),
        editable: true,
        json_string: false,
    }];
    while let Some(frame) = stack.pop() {
        match frame.value {
            Value::String(_) => {
                let location = Location {
                    pointer: frame.pointer,
                    editable: frame.editable,
                    json_string: frame.json_string,
                };
                record(&mut out, frame.path, location)?;
            },
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    stack.push(Frame {
                        value: item,
                        path: format!("{}[{index}]", frame.path),
                        pointer: format!("{}/{index}", frame.pointer),
                        editable: frame.editable,
                        json_string: false,
                    });
                }
            },
            Value::Object(map) => {
                let editable = frame.editable && !is_signed_block(map);
                for (key, item) in map {
                    record(
                        &mut out,
                        format!("{}.{key}.$key", frame.path),
                        Location::opaque(),
                    )?;
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    stack.push(Frame {
                        value: item,
                        path: format!("{}.{key}", frame.path),
                        pointer: format!("{}/{escaped}", frame.pointer),
                        editable: editable && !protected_key(key),
                        json_string: key == "arguments" && is_json_string(item),
                    });
                }
            },
            Value::Null | Value::Bool(_) | Value::Number(_) => {},
        }
    }
    Some(out)
}

fn record(out: &mut HashMap<String, Location>, path: String, location: Location) -> Option<()> {
    out.insert(path, location).is_none().then_some(())
}

// JSON: gateway recovery — walks the provider-bound wire body to redact located
// secrets.
fn is_json_string(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| serde_json::from_str::<Value>(s).is_ok())
}

// JSON: gateway recovery — walks the provider-bound wire body to redact located
// secrets.
fn is_signed_block(map: &Map<String, Value>) -> bool {
    let reasoning = matches!(
        map.get("type").and_then(Value::as_str),
        Some("thinking" | "redacted_thinking" | "reasoning")
    );
    map.contains_key("thoughtSignature")
        || map.contains_key("thought_signature")
        || (reasoning
            && ["signature", "data", "encrypted_content"]
                .iter()
                .any(|key| map.contains_key(*key)))
}

fn protected_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "type"
            | "role"
            | "name"
            | "model"
            | "tool_use_id"
            | "tool_call_id"
            | "call_id"
            | "signature"
            | "thoughtSignature"
            | "thought_signature"
            | "encrypted_content"
            | "data"
            | "url"
            | "file_id"
            | "file_data"
            | "mime_type"
            | "mimeType"
            | "media_type"
            | "encoding"
            | "format"
            | "previous_response_id"
    ) || key.ends_with("_id")
}
