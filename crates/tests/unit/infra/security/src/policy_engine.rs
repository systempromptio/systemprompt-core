use serde_json::json;
use systemprompt_governance::authz::types::{Decision, DenyReason, MatchedBy};
use systemprompt_governance::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_governance::policy::types::{
    AccessScope, AgentScope, GovernancePolicy, PolicyContext,
};
use systemprompt_governance::policy::{ChainEntryResult, GovernanceConfig, GovernanceEngine};
use systemprompt_governance::register_governance_policy;
use systemprompt_identifiers::{CallId, McpToolName, PolicyId, SessionId, UserId};

#[derive(Debug)]
struct StaticAllow;

impl GovernancePolicy for StaticAllow {
    fn id(&self) -> PolicyId {
        PolicyId::new("t_allow")
    }
    fn name(&self) -> &'static str {
        "test allow"
    }
    fn description(&self) -> &'static str {
        "always allows"
    }
    fn evaluate(&self, _ctx: &PolicyContext<'_>) -> Decision {
        Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new("t_allow"),
                detail: std::borrow::Cow::Borrowed("test pass"),
            },
        }
    }
}

#[derive(Debug)]
struct StaticDeny;

impl GovernancePolicy for StaticDeny {
    fn id(&self) -> PolicyId {
        PolicyId::new("t_deny")
    }
    fn name(&self) -> &'static str {
        "test deny"
    }
    fn description(&self) -> &'static str {
        "always denies"
    }
    fn evaluate(&self, _ctx: &PolicyContext<'_>) -> Decision {
        Decision::Deny {
            reason: DenyReason::PolicyViolation {
                policy: "t_deny".to_owned(),
                detail: std::borrow::Cow::Borrowed("blocked"),
            },
        }
    }
}

register_governance_policy!("t_allow", |_| Ok(Box::new(StaticAllow)));
register_governance_policy!("t_deny", |_| Ok(Box::new(StaticDeny)));

fn ctx<'a>(
    session: &'a SessionId,
    user: &'a UserId,
    input: &'a GovernedInput,
    call: &'a CallId,
) -> PolicyContext<'a> {
    PolicyContext {
        target: GovernedTarget::Tool {
            tool: McpToolName::try_new("read_file").expect("valid McpToolName"),
        },
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        roles: &[],
        session_id: session,
        user_id: user,
        input,
        call_id: call,
        rate_counts: &NO_RATE_COUNTS,
    }
}

fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(
        &GovernanceConfig::parse(yaml).unwrap(),
        Some(crate::offline_rate_windows()),
    )
    .unwrap()
}

fn entry_result(
    evaluation: &systemprompt_governance::policy::Evaluation,
    id: &str,
) -> ChainEntryResult {
    evaluation
        .chain
        .iter()
        .find(|e| e.policy_id.as_str() == id)
        .unwrap_or_else(|| panic!("no chain entry for {id}"))
        .result
}

#[test]
fn deny_short_circuits_and_later_entries_record_skip() {
    let e = engine("governance:\n  policies:\n    - id: t_deny\n    - id: t_allow\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u1");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(evaluation.decision, Decision::Deny { .. }));
    assert_eq!(entry_result(&evaluation, "t_deny"), ChainEntryResult::Fail);
    assert_eq!(entry_result(&evaluation, "t_allow"), ChainEntryResult::Skip);
}

#[test]
fn trace_preserves_declaration_order_and_pass_entries() {
    let e = engine("governance:\n  policies:\n    - id: t_allow\n    - id: t_deny\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u2");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(evaluation.decision, Decision::Deny { .. }));
    assert_eq!(evaluation.chain[0].policy_id.as_str(), "t_allow");
    assert_eq!(evaluation.chain[0].result, ChainEntryResult::Pass);
    assert_eq!(evaluation.chain[0].detail, "test pass");
    assert_eq!(evaluation.chain[1].policy_id.as_str(), "t_deny");
    assert_eq!(evaluation.chain[1].result, ChainEntryResult::Fail);
}

#[test]
fn disabled_policy_records_disabled_and_does_not_evaluate() {
    let e = engine(
        "governance:\n  policies:\n    - id: t_deny\n      enabled: false\n    - id: t_allow\n",
    );
    let sid = SessionId::generate();
    let uid = UserId::new("u3");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
    assert_eq!(
        entry_result(&evaluation, "t_deny"),
        ChainEntryResult::Disabled
    );
}

#[test]
fn unknown_config_id_rejects_engine_construction() {
    let config = GovernanceConfig::parse(
        "governance:\n  policies:\n    - id: no_such_policy\n    - id: t_allow\n",
    )
    .unwrap();
    let error =
        GovernanceEngine::from_config(&config, Some(crate::offline_rate_windows())).unwrap_err();
    assert!(matches!(
        error,
        systemprompt_governance::policy::GovernanceEngineError::UnknownPolicyId { id }
        if id == "no_such_policy"
    ));
}

#[test]
fn unmentioned_registered_policies_are_appended_disabled() {
    let e = engine("governance:\n  policies:\n    - id: t_allow\n");
    let (cfg, _) = e
        .policies()
        .find(|(cfg, _)| cfg.id == "t_deny")
        .expect("registered-but-unmentioned policy must appear in the chain");
    assert!(!cfg.enabled);

    let sid = SessionId::generate();
    let uid = UserId::new("u4");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();
    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(evaluation.decision, Decision::Allow { .. }));
    assert_eq!(
        entry_result(&evaluation, "t_deny"),
        ChainEntryResult::Disabled
    );
}

#[test]
fn all_disabled_chain_allows_with_default_included() {
    let e = engine("governance:\n  policies:\n    - id: t_allow\n      enabled: false\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u5");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(
        evaluation.decision,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    ));
    assert!(
        evaluation
            .chain
            .iter()
            .all(|e| e.result == ChainEntryResult::Disabled)
    );
}

#[test]
fn the_master_switch_allows_without_evaluating_any_policy() {
    let e = engine("governance:\n  enabled: false\n  policies:\n    - id: t_deny\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u7");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(
        evaluation.decision,
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded
        }
    ));
    assert_eq!(
        entry_result(&evaluation, "t_deny"),
        ChainEntryResult::Disabled
    );
}

#[test]
fn the_master_switch_still_traces_every_configured_policy() {
    let e =
        engine("governance:\n  enabled: false\n  policies:\n    - id: t_deny\n    - id: t_allow\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u8");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    for id in ["t_deny", "t_allow"] {
        assert_eq!(entry_result(&evaluation, id), ChainEntryResult::Disabled);
    }
    assert!(
        evaluation
            .chain
            .iter()
            .all(|e| e.duration_ms.abs() < f64::EPSILON)
    );
}

#[test]
fn the_master_switch_defaults_to_on_when_the_key_is_absent() {
    let e = engine("governance:\n  policies:\n    - id: t_deny\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u9");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    assert!(matches!(evaluation.decision, Decision::Deny { .. }));
}

#[test]
fn default_config_builds_the_four_builtins_enabled() {
    let e = GovernanceEngine::from_config(
        &GovernanceConfig::defaults(),
        Some(crate::offline_rate_windows()),
    )
    .unwrap();
    for id in ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"] {
        let (cfg, _) = e
            .policies()
            .find(|(cfg, _)| cfg.id == id)
            .unwrap_or_else(|| panic!("builtin {id} missing from default engine"));
        assert!(cfg.enabled, "builtin {id} must default to enabled");
    }
}

#[test]
fn skipped_entries_carry_zero_duration() {
    let e = engine("governance:\n  policies:\n    - id: t_deny\n    - id: t_allow\n");
    let sid = SessionId::generate();
    let uid = UserId::new("u6");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();

    let evaluation = e.evaluate(&ctx(&sid, &uid, &input, &call));
    let skipped = evaluation
        .chain
        .iter()
        .find(|e| e.result == ChainEntryResult::Skip)
        .unwrap();
    assert!(skipped.duration_ms.abs() < f64::EPSILON);
}

static NO_RATE_COUNTS: systemprompt_governance::policy::RateCounts =
    systemprompt_governance::policy::RateCounts::empty();

fn engine_from(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(
        &systemprompt_governance::policy::GovernanceConfig::parse(yaml).expect("parses"),
        Some(crate::offline_rate_windows()),
    )
    .expect("valid config")
}

#[test]
fn the_default_chain_is_warn_only() {
    let e = GovernanceEngine::from_config(
        &systemprompt_governance::policy::GovernanceConfig::defaults(),
        Some(crate::offline_rate_windows()),
    )
    .expect("defaults build");
    assert!(
        e.policies()
            .filter(
                |(cfg, _)| ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"]
                    .contains(&cfg.id.as_str())
            )
            .all(|(cfg, _)| cfg.enabled && cfg.mode.is_warn())
    );
    assert!(!e.enforces_prompt_secrets());
}

#[test]
fn a_declared_chain_builds_with_its_policy_first() {
    let e = engine_from("governance:\n  policies:\n    - id: t_deny\n");
    let rendered = format!("{e:?}");
    assert!(
        rendered.contains("t_deny") && rendered.contains("rate_windows: true"),
        "{rendered}"
    );
    let (cfg, _) = e.policies().next().expect("declared policy first");
    assert_eq!(cfg.id.as_str(), "t_deny");
    assert!(cfg.enabled);
}
