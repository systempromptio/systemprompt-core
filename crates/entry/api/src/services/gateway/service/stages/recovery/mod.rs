//! Prompt governance with transactional secret sanitization.
//!
//! [`govern_prompt`] evaluates the exact provider-bound body rather than the
//! canonical request, so what is governed is what is sent. When the secret
//! scanner denies, `repair` redacts the located spans in the JSON body and
//! mirrors the change into the canonical views (`canonical`); the engine
//! re-verifies the repaired body before the request proceeds. A repair that
//! cannot be made safely — a signed block, a protected key, an ambiguous
//! path, a body too large to inspect completely — leaves the denial in place,
//! with its detail scrubbed so no credential reaches the client or the audit
//! row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod canonical;
mod repair;

use std::borrow::Cow;

use axum::response::Response;
use http::HeaderValue;
// JSON: the provider wire payload is inspected as raw JSON to tell an
// uninspectable body from an empty one.
use serde_json::Value;
use systemprompt_identifiers::PolicyId;
use systemprompt_models::wire::canonical::CanonicalRequest;
use systemprompt_models::wire::inspect::{self, ForwardedSurface, SurfaceBudget};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::secrets::SecretFinding;
use systemprompt_security::policy::{
    ChainEntryOutcome, ChainEntryResult, Evaluation, GovernanceEngine, GovernedInput,
    PolicyContext, SECRET_SCAN_ID,
};

#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub use self::repair::repair_prompt;
use crate::services::gateway::protocol::outbound::PreparedBody;
use crate::services::gateway::service::RECOVERY_COUNT_HEADER;

const INCOMPLETE_INSPECTION_MESSAGE: &str = "Prompt could not be completely inspected; shorten \
                                             the conversation or remove large attachments \
                                             before retrying";
const UNSAFE_REPAIR_DETAIL: &str =
    "Secret content could not be safely sanitized; repair provider_payload before retrying";
const FALLBACK_LOCATION: &str = "provider_payload";

#[derive(Debug)]
#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub struct PromptRecovery {
    pub evaluation: Evaluation,
    pub recovery_count: usize,
    pub recovery_locations: Vec<String>,
}

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
pub fn govern_prompt(
    engine: &GovernanceEngine,
    ctx: &PolicyContext<'_>,
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
) -> PromptRecovery {
    request.forwarded_surface = inspect::string_leaves(&body.bytes, inspection_budget());
    let input = governed_input(&request.forwarded_surface);
    let ctx = ctx.with_input(&input);
    let mut recovery_count = 0;
    let mut recovery_locations = Vec::new();
    let mut repaired = None;
    let mut evaluation = if engine.enforces_prompt_secrets()
        && inspection_incomplete(&request.forwarded_surface, &body.bytes)
    {
        incomplete_inspection_denial()
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

fn inspection_incomplete(surface: &ForwardedSurface, bytes: &[u8]) -> bool {
    surface.truncated() || (surface.is_empty() && serde_json::from_slice::<Value>(bytes).is_err())
}

fn incomplete_inspection_denial() -> Evaluation {
    Evaluation {
        decision: Decision::Deny {
            reason: DenyReason::PolicyViolation {
                policy: SECRET_SCAN_ID.to_owned(),
                detail: Cow::Borrowed(INCOMPLETE_INSPECTION_MESSAGE),
            },
        },
        chain: vec![ChainEntryOutcome {
            policy_id: PolicyId::new(SECRET_SCAN_ID),
            result: ChainEntryResult::Fail,
            detail: "Incomplete prompt inspection".to_owned(),
            duration_ms: 0.0,
        }],
    }
}

fn sanitize_denial(evaluation: &mut Evaluation, recovery_locations: &[String]) {
    let Decision::Deny {
        reason: DenyReason::SecretLeak { location, .. },
    } = &mut evaluation.decision
    else {
        return;
    };
    "[REDACTED]".clone_into(&mut location.redacted);
    location.path = recovery_locations
        .first()
        .cloned()
        .unwrap_or_else(|| FALLBACK_LOCATION.to_owned());
    for entry in &mut evaluation.chain {
        if entry.result == ChainEntryResult::Fail {
            UNSAFE_REPAIR_DETAIL.clone_into(&mut entry.detail);
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
                format!("{FALLBACK_LOCATION}.parts[{}]", finding.source.part_index)
            } else {
                source.path.clone()
            }
        })
        .collect();
    locations.sort();
    locations.dedup();
    locations
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(unreachable_pub, reason = "Re-exported for recovery regression tests")
)]
pub fn attach_recovery_count(response: &mut Response, count: usize) {
    if count > 0 {
        response
            .headers_mut()
            .insert(RECOVERY_COUNT_HEADER, HeaderValue::from(count));
    }
}
