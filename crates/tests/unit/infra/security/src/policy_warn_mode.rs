use serde_json::json;
use systemprompt_identifiers::{CallId, McpToolName, PolicyId, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, DecisionTag, DenyReason, MatchedBy};
use systemprompt_security::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_security::policy::types::{
    AccessScope, AgentScope, GovernancePolicy, PolicyContext,
};
use systemprompt_security::policy::{
    ChainEntryResult, GovernanceConfig, GovernanceConfigError, GovernanceEngine, PolicyMode,
};
use systemprompt_security::register_governance_policy;

#[derive(Debug)]
struct WarnModeDeny;

impl GovernancePolicy for WarnModeDeny {
    fn id(&self) -> PolicyId {
        PolicyId::new("t_warnable")
    }
    fn name(&self) -> &'static str {
        "test warnable deny"
    }
    fn description(&self) -> &'static str {
        "always denies, so warn mode has something to soften"
    }
    fn evaluate(&self, _ctx: &PolicyContext<'_>) -> Decision {
        Decision::Deny {
            reason: DenyReason::PolicyViolation {
                policy: "t_warnable".to_owned(),
                detail: std::borrow::Cow::Borrowed("entropy backstop"),
            },
        }
    }
}

#[derive(Debug)]
struct WarnModeAllow;

impl GovernancePolicy for WarnModeAllow {
    fn id(&self) -> PolicyId {
        PolicyId::new("t_warnable_allow")
    }
    fn name(&self) -> &'static str {
        "test warnable allow"
    }
    fn description(&self) -> &'static str {
        "always allows"
    }
    fn evaluate(&self, _ctx: &PolicyContext<'_>) -> Decision {
        Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new("t_warnable_allow"),
                detail: std::borrow::Cow::Borrowed("ok"),
            },
        }
    }
}

register_governance_policy!("t_warnable", |_| Box::new(WarnModeDeny));
register_governance_policy!("t_warnable_allow", |_| Box::new(WarnModeAllow));

fn ctx<'a>(
    session: &'a SessionId,
    user: &'a UserId,
    input: &'a GovernedInput,
    call: &'a CallId,
) -> PolicyContext<'a> {
    PolicyContext {
        target: GovernedTarget::Tool {
            tool: McpToolName::new("read_file"),
        },
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        session_id: session,
        user_id: user,
        input,
        call_id: call,
    }
}

fn evaluate(yaml: &str) -> systemprompt_security::policy::Evaluation {
    let engine = GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap());
    let sid = SessionId::generate();
    let uid = UserId::new("warn-user");
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    let call = CallId::generate();
    engine.evaluate(&ctx(&sid, &uid, &input, &call))
}

fn entry_result(
    evaluation: &systemprompt_security::policy::Evaluation,
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
fn mode_defaults_to_enforce_everywhere() {
    let cfg =
        GovernanceConfig::parse("governance:\n  policies:\n    - id: secret_scan\n").unwrap();
    assert_eq!(cfg.mode, PolicyMode::Enforce);
    assert_eq!(cfg.policies[0].mode, PolicyMode::Enforce);
    assert!(GovernanceConfig::defaults()
        .policies
        .iter()
        .all(|p| p.mode == PolicyMode::Enforce));
}

#[test]
fn the_top_level_mode_is_inherited_and_a_policy_may_override_it() {
    let yaml = concat!(
        "governance:\n",
        "  mode: warn\n",
        "  policies:\n",
        "    - id: secret_scan\n",
        "    - id: scope_check\n",
        "      mode: enforce\n"
    );
    let cfg = GovernanceConfig::parse(yaml).unwrap();
    assert_eq!(cfg.mode, PolicyMode::Warn);
    assert_eq!(cfg.policies[0].mode, PolicyMode::Warn);
    assert_eq!(cfg.policies[1].mode, PolicyMode::Enforce);
}

#[test]
fn an_unknown_mode_is_a_parse_error_rather_than_a_silent_default() {
    let err = GovernanceConfig::parse("governance:\n  mode: warnn\n  policies: []\n").unwrap_err();
    assert!(matches!(err, GovernanceConfigError::InvalidMode { .. }));

    let per_policy = GovernanceConfig::parse(
        "governance:\n  policies:\n    - id: secret_scan\n      mode: observe\n",
    )
    .unwrap_err();
    assert!(matches!(
        per_policy,
        GovernanceConfigError::InvalidMode { .. }
    ));
}

#[test]
fn a_warn_mode_deny_permits_the_call_and_records_the_reason() {
    let evaluation =
        evaluate("governance:\n  mode: warn\n  policies:\n    - id: t_warnable\n");
    let Decision::Warn { reason } = &evaluation.decision else {
        panic!("expected a warn verdict, got {:?}", evaluation.decision);
    };
    assert!(reason.to_string().contains("entropy backstop"));
    assert!(evaluation.decision.permits());
    assert_eq!(evaluation.decision.tag(), DecisionTag::Warn);
    assert_eq!(
        entry_result(&evaluation, "t_warnable"),
        ChainEntryResult::Warn
    );
}

#[test]
fn warn_does_not_halt_the_chain() {
    let evaluation = evaluate(
        "governance:\n  policies:\n    - id: t_warnable\n      mode: warn\n    - id: \
         t_warnable_allow\n",
    );
    assert_eq!(
        entry_result(&evaluation, "t_warnable_allow"),
        ChainEntryResult::Pass,
        "a warn must not skip the policies after it — the report wants every finding"
    );
}

#[test]
fn an_enforcing_policy_after_a_warning_one_still_denies() {
    let evaluation = evaluate(
        "governance:\n  policies:\n    - id: t_warnable\n      mode: warn\n    - id: t_deny\n",
    );
    assert!(
        matches!(evaluation.decision, Decision::Deny { .. }),
        "warn mode on one policy must not disarm the rest of the chain"
    );
    assert_eq!(
        entry_result(&evaluation, "t_warnable"),
        ChainEntryResult::Warn
    );
    assert_eq!(entry_result(&evaluation, "t_deny"), ChainEntryResult::Fail);
}

#[test]
fn enforce_mode_still_denies_and_still_halts() {
    let evaluation = evaluate("governance:\n  policies:\n    - id: t_warnable\n");
    assert!(matches!(evaluation.decision, Decision::Deny { .. }));
    assert_eq!(
        entry_result(&evaluation, "t_warnable"),
        ChainEntryResult::Fail
    );
}

#[test]
fn the_decision_tag_string_is_the_value_the_check_constraint_allows() {
    assert_eq!(DecisionTag::Warn.as_str(), "warn");
}
