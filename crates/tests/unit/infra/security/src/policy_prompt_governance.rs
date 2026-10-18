//! Governance of a `GovernedTarget::Prompt` — the inference plane.
//!
//! The `/v1/messages` gateway runs the same chain as the MCP tool-call
//! webhook, but against a prompt rather than tool arguments. These tests pin
//! the two properties that make that safe: an operator's configured patterns
//! reach inference (the capability the hardcoded safety scanner lacks), and
//! every enforcement point shares one engine, so the rate limiter charges one
//! budget rather than one per plane.

use serde_json::json;
use systemprompt_identifiers::{CallId, SessionId, UserId};
use systemprompt_governance::authz::types::{Decision, DenyReason};
use systemprompt_governance::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_governance::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_governance::policy::{
    ChainEntryResult, GovernanceConfig, GovernanceEngine, GovernanceEngineError, PROMPT_TARGET_NAME,
};

const EXTRA_PATTERN: &str = "governance:\n  policies:\n    - id: secret_scan\n      patterns:\n        - id: demo-key\n          name: Demo Key\n          regex: 'XDEMO-[0-9]+'\n";

fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(
        &GovernanceConfig::parse(yaml).unwrap(),
        Some(crate::offline_rate_windows()),
    )
    .unwrap()
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
        roles: &[],
        session_id: session,
        user_id: user,
        input,
        call_id: call,
        rate_counts: &NO_RATE_COUNTS,
    }
}

// Why: this pins that an operator-configured pattern governs inference input.
#[test]
fn operator_patterns_deny_an_inference_prompt() {
    let e = engine(EXTRA_PATTERN);
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-extra"),
        CallId::generate(),
    );
    let input = GovernedInput::prompt_text("here is my key XDEMO-1234 please use it".to_owned());

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
    let input = GovernedInput::prompt_text("summarise the quarterly report".to_owned());

    let evaluation = e.evaluate(&prompt_ctx(&input, &session, &user, &call));

    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
}

// Why: on a Prompt target the two tool-shaped policies cannot judge anything,
// but they must still appear in the trace — the audit row records the whole
// evaluation order, not just the policies that had an opinion.
#[test]
fn tool_shaped_policies_still_appear_in_a_prompt_chain_trace() {
    let e = engine(
        "governance:\n  policies:\n    - id: secret_scan\n      mode: warn\n    - id: \
         scope_check\n    - id: tool_blocklist\n",
    );
    let (session, user, call) = (
        SessionId::generate(),
        UserId::new("u-prompt-chain"),
        CallId::generate(),
    );
    let input = GovernedInput::prompt_text("hello".to_owned());

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

#[test]
fn a_missing_services_governance_config_builds_the_warn_only_defaults() {
    let root = tempfile::tempdir().unwrap();
    let engine =
        GovernanceEngine::from_services_root(root.path(), Some(crate::offline_rate_windows()))
            .expect("defaults");
    assert!(
        !engine.enforces_prompt_secrets(),
        "the default chain is warn-only, so prompt secrets are not enforced"
    );
}

#[test]
fn a_rejected_services_governance_config_refuses_the_engine() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("governance")).unwrap();
    std::fs::write(
        root.path().join("governance/config.yaml"),
        "enabled: [not, a, bool]",
    )
    .unwrap();
    let err =
        GovernanceEngine::from_services_root(root.path(), Some(crate::offline_rate_windows()))
            .expect_err("rejected");
    assert!(
        matches!(err, GovernanceEngineError::ConfigRejected { .. }),
        "{err}"
    );
}

// Why: the gateway derives call_id from the ai-request id precisely so a
// re-evaluated call is charged once. A prompt and a tool call share that
// contract, so it is pinned on the prompt path too.
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
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({ "note": "XDEMO-1234" })));

    let evaluation = e.evaluate(&PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::Unknown,
        roles: &[],
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
        rate_counts: &NO_RATE_COUNTS,
    });

    assert!(matches!(
        &evaluation.decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { .. }
        }
    ));
}

static NO_RATE_COUNTS: systemprompt_governance::policy::RateCounts =
    systemprompt_governance::policy::RateCounts::empty();
