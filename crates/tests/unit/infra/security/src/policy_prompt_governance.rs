//! Governance of a `GovernedTarget::Prompt` — the inference plane.
//!
//! The `/v1/messages` gateway runs the same chain as the MCP tool-call
//! webhook, but against a prompt rather than tool arguments. These tests pin
//! the two properties that make that safe: an operator's `extra_patterns`
//! reach inference (the capability the hardcoded safety scanner lacks), and
//! every enforcement point shares one engine, so the rate limiter charges one
//! budget rather than one per plane.

use serde_json::json;
use systemprompt_identifiers::{CallId, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_security::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_security::policy::{
    ChainEntryResult, GovernanceConfig, GovernanceEngine, PROMPT_TARGET_NAME,
};

const EXTRA_PATTERN: &str = "governance:\n  policies:\n    - id: secret_scan\n      extra_patterns:\n        - name: Demo Key\n          prefix: \"XDEMO-\"\n";

fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap())
}

fn prompt_ctx<'a>(
    input: &'a GovernedInput,
    session: &'a SessionId,
    user: &'a UserId,
    call: &'a CallId,
) -> PolicyContext<'a> {
    PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::Unknown,
        session_id: session,
        user_id: user,
        input,
        call_id: call,
    }
}

// Why: this is the capability the gateway's hardcoded SECRET_PATTERNS scanner
// cannot offer — an operator-configured pattern denying an inference request.
#[test]
fn operator_extra_patterns_deny_an_inference_prompt() {
    let e = engine(EXTRA_PATTERN);
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-extra"),
        CallId::generate(),
    );
    let input = GovernedInput::prompt("here is my key XDEMO-1234 please use it".to_owned());

    let evaluation = e.evaluate(&prompt_ctx(&input, &session, &user, &call));

    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { pattern_id, .. }
        } if pattern_id.as_str() == "demo-key"
    ));
}

#[test]
fn a_clean_inference_prompt_is_allowed() {
    let e = engine(EXTRA_PATTERN);
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-clean"),
        CallId::generate(),
    );
    let input = GovernedInput::prompt("summarise the quarterly report".to_owned());

    let evaluation = e.evaluate(&prompt_ctx(&input, &session, &user, &call));

    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

// Why: on a Prompt target the two tool-shaped policies cannot judge anything,
// but they must still appear in the trace — the audit row records the whole
// evaluation order, not just the policies that had an opinion.
#[test]
fn tool_shaped_policies_still_appear_in_a_prompt_chain_trace() {
    let e = engine(
        "governance:\n  policies:\n    - id: secret_scan\n    - id: scope_check\n    - id: \
         tool_blocklist\n    - id: rate_limit\n",
    );
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-chain"),
        CallId::generate(),
    );
    let input = GovernedInput::prompt("hello".to_owned());

    let evaluation = e.evaluate(&prompt_ctx(&input, &session, &user, &call));

    let ids: Vec<&str> = evaluation
        .chain
        .iter()
        .map(|entry| entry.policy_id.as_str())
        .collect();
    assert!(ids.contains(&"scope_check"), "got {ids:?}");
    assert!(ids.contains(&"tool_blocklist"), "got {ids:?}");
    assert!(
        evaluation
            .chain
            .iter()
            .all(|entry| entry.result != ChainEntryResult::Fail),
        "a clean prompt must not fail any entry; got {evaluation:?}"
    );
}

#[test]
fn a_prompt_target_names_itself_in_the_audit_row() {
    assert_eq!(GovernedTarget::Prompt.as_str(), PROMPT_TARGET_NAME);
}

// Why: the rate limiter's buckets are instance-scoped, so two engines would
// give the gateway and the MCP webhook a budget each and silently double every
// operator limit. This is the test that pins "one engine, one budget".
#[test]
fn the_global_engine_is_one_shared_instance() {
    let first: &'static GovernanceEngine = GovernanceEngine::global();
    let second: &'static GovernanceEngine = GovernanceEngine::global();

    assert!(
        std::ptr::eq(first, second),
        "every enforcement point must observe the same engine, and so the same limiter state"
    );
}

// Why: the gateway derives call_id from the ai-request id precisely so a
// re-evaluated call is charged once. A prompt and a tool call share that
// contract, so it is pinned on the prompt path too.
#[test]
fn re_evaluating_one_prompt_charges_the_limiter_once() {
    let e = engine(
        "governance:\n  policies:\n    - id: rate_limit\n      requests_per_window: 2\n      \
         window_secs: 60\n",
    );
    let (session, user) = (SessionId::generate(), UserId::new("u-prompt-idem"));
    let input = GovernedInput::prompt("hello".to_owned());
    let call = CallId::generate();

    for _ in 0..5 {
        let decision = e
            .evaluate(&prompt_ctx(&input, &session, &user, &call))
            .decision;
        assert!(
            matches!(decision, Decision::Allow { .. }),
            "the same call_id re-evaluated must stay allowed, not accumulate charges"
        );
    }

    // Why: a genuinely different call still consumes the remaining budget, so
    // the idempotency above is per-call and not a disabled limiter.
    let second = CallId::generate();
    assert!(matches!(
        e.evaluate(&prompt_ctx(&input, &session, &user, &second))
            .decision,
        Decision::Allow { .. }
    ));
    let third = CallId::generate();
    assert!(matches!(
        e.evaluate(&prompt_ctx(&input, &session, &user, &third))
            .decision,
        Decision::Deny {
            reason: DenyReason::RateLimitExceeded { .. }
        }
    ));
}

// Why: a tool-arguments input on a Prompt-governed engine must still scan —
// the gateway will grow a path that governs `tool_use` blocks inside a request
// body, and this pins that the scanner is not target-gated.
#[test]
fn secret_scanning_is_not_gated_on_the_target_kind() {
    let e = engine(EXTRA_PATTERN);
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-tool"),
        CallId::generate(),
    );
    let input =
        GovernedInput::tool_arguments(McpToolInput::new(json!({ "note": "XDEMO-1234" })));

    let evaluation = e.evaluate(&PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::Unknown,
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
    });

    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { .. }
        }
    ));
}
