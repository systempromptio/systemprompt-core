//! Transactional repair of the exact provider-bound JSON and its canonical text
//! views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use bytes::Bytes;
// JSON: provider wire payloads contain arbitrary client-defined tool arguments
// and metadata.
use serde_json::Value;
use systemprompt_models::wire::canonical::CanonicalRequest;

use super::recovery_canonical::replace_canonical;
use systemprompt_models::wire::inspect::{self, ForwardedSurface, SurfaceBudget};
use systemprompt_security::policy::GovernedInput;
use systemprompt_security::policy::secrets::{SecretFinding, redact_spans};

use super::super::super::protocol::outbound::PreparedBody;

pub(super) fn inspection_budget() -> SurfaceBudget {
    SurfaceBudget {
        leaf_bytes: 2 * 1024 * 1024,
        ..SurfaceBudget::default()
    }
}

pub(super) fn governed_input(surface: &ForwardedSurface) -> GovernedInput {
    GovernedInput::prompt_parts(
        surface
            .leaves()
            .iter()
            .map(|leaf| (format!("forwarded.{}", leaf.path), leaf.value.clone())),
    )
}

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
        let redacted = redact_spans(value, spans.clone())?;
        for span in spans {
            replacements.push((
                value.get(span)?.to_owned(),
                systemprompt_security::policy::secrets::REDACTION_MARKER.to_owned(),
            ));
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

#[derive(Debug)]
#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub struct PromptRecovery {
    pub evaluation: systemprompt_security::policy::Evaluation,
    pub recovery_count: usize,
    pub recovery_locations: Vec<String>,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub fn govern_prompt(
    engine: &systemprompt_security::policy::GovernanceEngine,
    ctx: &systemprompt_security::policy::PolicyContext<'_>,
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
) -> PromptRecovery {
    use std::borrow::Cow;
    use systemprompt_identifiers::PolicyId;
    use systemprompt_security::authz::types::{Decision, DenyReason};
    use systemprompt_security::policy::{
        ChainEntryOutcome, ChainEntryResult, Evaluation, PolicyContext,
    };
    request.forwarded_surface = inspect::string_leaves(&body.bytes, inspection_budget());
    let input = governed_input(&request.forwarded_surface);
    let ctx = PolicyContext {
        input: &input,
        target: ctx.target.clone(),
        agent_scope: ctx.agent_scope.clone(),
        access_scope: ctx.access_scope,
        session_id: ctx.session_id,
        user_id: ctx.user_id,
        call_id: ctx.call_id,
    };
    let mut recovery_count = 0;
    let mut recovery_locations = Vec::new();
    let mut repaired = None;
    let mut evaluation = if engine.enforces_prompt_secrets()
        && (request.forwarded_surface.truncated()
            || (request.forwarded_surface.is_empty()
                && serde_json::from_slice::<Value>(&body.bytes).is_err()))
    {
        Evaluation {
            decision: Decision::Deny {
                reason: DenyReason::PolicyViolation {
                    policy: "secret_scan".to_owned(),
                    detail: Cow::Borrowed(
                        "Prompt could not be completely inspected; shorten the conversation or remove large attachments before retrying",
                    ),
                },
            },
            chain: vec![ChainEntryOutcome {
                policy_id: PolicyId::new("secret_scan"),
                result: ChainEntryResult::Fail,
                detail: "Incomplete prompt inspection".to_owned(),
                duration_ms: 0.0,
            }],
        }
    } else {
        engine.evaluate_with_prompt_recovery(&ctx, |findings| {
            recovery_locations = safe_locations(ctx.input, findings);
            let (mut candidate_request, mut candidate_body) = repaired
                .take()
                .unwrap_or_else(|| (request.clone(), body.clone()));
            let input = repair_prompt(&mut candidate_request, &mut candidate_body, findings)?;
            recovery_count += findings.len();
            repaired = Some((candidate_request, candidate_body));
            Some(input)
        })
    };
    sanitize_denial(&mut evaluation, &recovery_locations);
    if evaluation.decision.permits()
        && let Some((candidate_request, candidate_body)) = repaired
    {
        *request = candidate_request;
        *body = candidate_body;
    } else {
        recovery_count = 0;
    }
    PromptRecovery {
        evaluation,
        recovery_count,
        recovery_locations,
    }
}

fn sanitize_denial(
    evaluation: &mut systemprompt_security::policy::Evaluation,
    recovery_locations: &[String],
) {
    use systemprompt_security::authz::types::{Decision, DenyReason};
    use systemprompt_security::policy::ChainEntryResult;
    if let Decision::Deny {
        reason: DenyReason::SecretLeak { location, .. },
    } = &mut evaluation.decision
    {
        "[REDACTED]".clone_into(&mut location.redacted);
        location.path = recovery_locations
            .first()
            .cloned()
            .unwrap_or_else(|| "provider_payload".to_owned());
        for entry in &mut evaluation.chain {
            if entry.result == ChainEntryResult::Fail {
                "Secret content could not be safely sanitized; repair provider_payload before retrying".clone_into(&mut entry.detail);
            }
        }
    }
}

fn safe_locations(input: &GovernedInput, findings: &[SecretFinding]) -> Vec<String> {
    let strings = input.strings();
    let secret_keys: Vec<_> = findings
        .iter()
        .filter_map(|finding| strings.get(finding.source.part_index))
        .filter(|source| source.path.ends_with(".$key"))
        .map(|source| source.value)
        .collect();
    let mut locations: Vec<_> = findings
        .iter()
        .map(|finding| {
            let source = &strings[finding.source.part_index];
            if secret_keys.iter().any(|key| source.path.contains(key)) {
                format!("provider_payload.parts[{}]", finding.source.part_index)
            } else {
                source.path.clone()
            }
        })
        .collect();
    locations.sort();
    locations.dedup();
    locations
}

struct Location {
    pointer: String,
    editable: bool,
    json_string: bool,
}

fn locations(root: &Value) -> Option<HashMap<String, Location>> {
    let mut out = HashMap::new();
    let mut stack = vec![(root, "$".to_owned(), String::new(), true, false)];
    while let Some((value, path, pointer, editable, json_string)) = stack.pop() {
        match value {
            Value::String(_) => {
                if out
                    .insert(
                        path,
                        Location {
                            pointer,
                            editable,
                            json_string,
                        },
                    )
                    .is_some()
                {
                    return None;
                }
            },
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    stack.push((
                        item,
                        format!("{path}[{index}]"),
                        format!("{pointer}/{index}"),
                        editable,
                        false,
                    ));
                }
            },
            Value::Object(map) => {
                let signed = map.contains_key("thoughtSignature")
                    || map.contains_key("thought_signature")
                    || matches!(
                        map.get("type").and_then(Value::as_str),
                        Some("thinking" | "redacted_thinking" | "reasoning")
                    ) && ["signature", "data", "encrypted_content"]
                        .iter()
                        .any(|key| map.contains_key(*key));
                for (key, item) in map {
                    if out
                        .insert(
                            format!("{path}.{key}.$key"),
                            Location {
                                pointer: String::new(),
                                editable: false,
                                json_string: false,
                            },
                        )
                        .is_some()
                    {
                        return None;
                    }
                    let escaped = key.replace('~', "~0").replace('/', "~1");
                    let editable = editable && !signed && !protected_key(key);
                    stack.push((
                        item,
                        format!("{path}.{key}"),
                        format!("{pointer}/{escaped}"),
                        editable,
                        key == "arguments"
                            && item
                                .as_str()
                                .is_some_and(|s| serde_json::from_str::<Value>(s).is_ok()),
                    ));
                }
            },
            Value::Null | Value::Bool(_) | Value::Number(_) => {},
        }
    }
    Some(out)
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

#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub fn attach_recovery_count(response: &mut axum::response::Response, count: usize) {
    if count > 0 {
        response.headers_mut().insert(
            super::super::RECOVERY_COUNT_HEADER,
            http::HeaderValue::from(count),
        );
    }
}
