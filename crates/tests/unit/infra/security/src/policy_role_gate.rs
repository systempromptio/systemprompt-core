//! `scope_check.role_gated` and `tool_blocklist.role_exemptions`: a role
//! named by a rule passes the tools it covers, and only those.

use serde_json::json;
use systemprompt_governance::authz::types::{Decision, DenyReason};
use systemprompt_governance::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_governance::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_governance::policy::{GovernanceConfig, GovernanceEngine};
use systemprompt_identifiers::{CallId, McpToolName, SessionId, UserId};

const YAML: &str = r#"
governance:
  mode: enforce
  policies:
    - id: scope_check
      enabled: true
      role_gated:
        - prefix: "mcp__odoo__"
          contains: ["delete", "erase"]
          roles: ["odoo_destructive"]
    - id: tool_blocklist
      enabled: true
      patterns: ["delete", "erase"]
      role_exemptions:
        - prefix: "mcp__odoo__"
          roles: ["odoo_destructive"]
"#;

fn decide(name: &str, scope: AccessScope, roles: &[String]) -> Decision {
    let engine = GovernanceEngine::from_config(
        &GovernanceConfig::parse(YAML).unwrap(),
        Some(crate::offline_rate_windows()),
    )
    .unwrap();
    let session = SessionId::generate();
    let user = UserId::new("role-gate-user");
    let call = CallId::generate();
    let input = GovernedInput::tool_arguments(McpToolInput::new(json!({})));
    engine
        .evaluate(&PolicyContext {
            target: GovernedTarget::Tool {
                tool: McpToolName::try_new(name).expect("valid McpToolName"),
            },
            agent_scope: AgentScope::User {
                user_id: user.clone(),
            },
            access_scope: scope,
            roles,
            session_id: &session,
            user_id: &user,
            input: &input,
            call_id: &call,
            rate_counts: &NO_RATE_COUNTS,
        })
        .decision
}

#[test]
fn a_gated_tool_is_refused_without_the_role() {
    let decision = decide(
        "mcp__odoo__lead_delete",
        AccessScope::User,
        &["user".to_owned()],
    );
    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::ScopeViolation { .. }
            }
        ),
        "{decision:?}"
    );
}

#[test]
fn the_role_passes_both_policies_for_its_prefix() {
    let roles = ["user".to_owned(), "odoo_destructive".to_owned()];
    let decision = decide("mcp__odoo__partner_erase", AccessScope::User, &roles);
    assert!(matches!(decision, Decision::Allow { .. }), "{decision:?}");
}

#[test]
fn the_role_does_not_exempt_other_servers() {
    let roles = ["odoo_destructive".to_owned()];
    let decision = decide("mcp__crm__lead_delete", AccessScope::User, &roles);
    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::ToolBlocked { .. }
            }
        ),
        "{decision:?}"
    );
}

#[test]
fn admin_passes_without_the_role() {
    let decision = decide("mcp__odoo__lead_delete", AccessScope::Admin, &[]);
    assert!(matches!(decision, Decision::Allow { .. }), "{decision:?}");
}

static NO_RATE_COUNTS: systemprompt_governance::policy::RateCounts =
    systemprompt_governance::policy::RateCounts::empty();
