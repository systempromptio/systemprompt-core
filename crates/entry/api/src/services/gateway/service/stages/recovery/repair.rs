//! In-place redaction of located secrets inside the provider-bound JSON body.
//!
//! Every string leaf is addressed twice: by the dotted inspection path the
//! scanner reports and by the JSON pointer used to edit it. A leaf is only
//! editable when nothing on its path is provider-signed or protocol-bearing;
//! a repair that would touch one of those, or whose edit no longer matches
//! what was scanned, is abandoned as a whole so a half-repaired body is never
//! forwarded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use bytes::Bytes;
// JSON: provider wire payloads contain arbitrary client-defined tool arguments
// and metadata.
use serde_json::{Map, Value};
use systemprompt_models::wire::canonical::CanonicalRequest;
use systemprompt_models::wire::inspect;
use systemprompt_security::policy::GovernedInput;
use systemprompt_security::policy::secrets::{REDACTION_MARKER, SecretFinding, redact_spans};

use super::canonical::replace_canonical;
use super::{governed_input, inspection_budget};
use crate::services::gateway::protocol::outbound::PreparedBody;

#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub fn repair_prompt(
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
    findings: &[SecretFinding],
) -> Option<GovernedInput> {
    let surface = inspect::string_leaves(&body.bytes, inspection_budget());
    if surface.truncated() || findings.is_empty() {
        return None;
    }
    let mut root: Value = serde_json::from_slice(&body.bytes).ok()?;
    let locations = locations(&root)?;
    let mut grouped: HashMap<usize, Vec<_>> = HashMap::new();
    for finding in findings {
        grouped
            .entry(finding.source.part_index)
            .or_default()
            .push(finding.span.clone());
    }
    let mut replacements = Vec::new();
    for (index, spans) in grouped {
        let leaf = surface.leaves().get(index)?;
        let location = locations.get(&leaf.path)?;
        if !location.editable {
            return None;
        }
        let slot = root.pointer_mut(&location.pointer)?;
        let value = slot.as_str()?;
        if value != leaf.value {
            return None;
        }
        let redacted = redact_spans(value, spans.iter().cloned())?;
        for span in spans {
            replacements.push((value.get(span)?.to_owned(), REDACTION_MARKER.to_owned()));
        }
        if location.json_string && serde_json::from_str::<Value>(&redacted).is_err() {
            return None;
        }
        replacements.push((value.to_owned(), redacted.clone()));
        *slot = Value::String(redacted);
    }
    let bytes = Bytes::from(serde_json::to_vec(&root).ok()?);
    let surface = inspect::string_leaves(&bytes, inspection_budget());
    if surface.truncated() {
        return None;
    }
    replacements.sort_by_key(|(old, _)| std::cmp::Reverse(old.len()));
    let mut repaired = request.clone();
    replace_canonical(&mut repaired, &replacements);
    let input = governed_input(&surface);
    repaired.forwarded_surface = surface;
    *request = repaired;
    body.bytes = bytes;
    Some(input)
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
    value: &'a Value,
    path: String,
    pointer: String,
    editable: bool,
    json_string: bool,
}

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

fn is_json_string(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| serde_json::from_str::<Value>(s).is_ok())
}

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
