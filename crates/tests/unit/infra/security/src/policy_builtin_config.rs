use serde_json::json;
use systemprompt_identifiers::{CallId, McpToolName, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, DenyReason};
use systemprompt_security::policy::governed::{GovernedInput, GovernedTarget, McpToolInput};
use systemprompt_security::policy::types::{AccessScope, AgentScope, PolicyContext};
use systemprompt_security::policy::{GovernanceConfig, GovernanceEngine};

fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap())
}

struct Call {
    session: SessionId,
    user: UserId,
    call: CallId,
}

impl Call {
    fn new(user: &str) -> Self {
        Self {
            session: SessionId::generate(),
            user: UserId::new(user),
            call: CallId::generate(),
        }
    }

    fn ctx<'a>(
        &'a self,
        target: &GovernedTarget,
        scope: AccessScope,
        input: &'a GovernedInput,
    ) -> PolicyContext<'a> {
        PolicyContext {
            target: target.clone(),
            agent_scope: AgentScope::User {
                user_id: self.user.clone(),
            },
            access_scope: scope,
            session_id: &self.session,
            user_id: &self.user,
            input,
            call_id: &self.call,
        }
    }
}

fn tool(name: &str) -> GovernedTarget {
    GovernedTarget::Tool {
        tool: McpToolName::new(name),
    }
}

fn args(v: serde_json::Value) -> GovernedInput {
    GovernedInput::tool_arguments(McpToolInput::new(v))
}

const ONLY_BLOCKLIST: &str = r#"
governance:
  enabled: true
  policies:
    - id: tool_blocklist
      enabled: true
      patterns: ["wire_transfer", "rm_rf"]
"#;

#[test]
fn configured_tool_blocklist_patterns_replace_the_built_in_defaults() {
    let eng = engine(ONLY_BLOCKLIST);
    let call = Call::new("user-blocklist-config");
    let input = args(json!({}));

    let target = tool("wire_transfer_funds");
    let denied = eng.evaluate(&call.ctx(&target, AccessScope::User, &input));
    match denied.decision {
        Decision::Deny {
            reason: DenyReason::ToolBlocked { tool, list_id },
        } => {
            assert_eq!(tool.as_str(), "wire_transfer_funds");
            assert_eq!(list_id, "wire_transfer");
        },
        other => panic!("configured pattern must deny, got {other:?}"),
    }

    let default_pattern = tool("delete_everything");
    let allowed = eng.evaluate(&call.ctx(&default_pattern, AccessScope::User, &input));
    assert!(
        matches!(allowed.decision, Decision::Allow { .. }),
        "the built-in `delete` pattern must be gone once patterns are configured"
    );
}

#[test]
fn admin_scope_survives_a_configured_tool_blocklist_match() {
    let eng = engine(ONLY_BLOCKLIST);
    let call = Call::new("user-blocklist-admin");
    let input = args(json!({}));
    let target = tool("rm_rf_root");

    let decision = eng
        .evaluate(&call.ctx(&target, AccessScope::Admin, &input))
        .decision;
    assert!(
        matches!(decision, Decision::Allow { .. }),
        "admin scope must bypass the blocklist, got {decision:?}"
    );
}

#[test]
fn an_empty_patterns_list_falls_back_to_the_built_in_defaults() {
    let eng = engine(
        r#"
governance:
  enabled: true
  policies:
    - id: tool_blocklist
      enabled: true
      patterns: []
"#,
    );
    let call = Call::new("user-blocklist-empty");
    let input = args(json!({}));
    let target = tool("drop_table");

    let decision = eng
        .evaluate(&call.ctx(&target, AccessScope::User, &input))
        .decision;
    assert!(
        matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::ToolBlocked { .. }
            }
        ),
        "an empty configured list must not disable the policy, got {decision:?}"
    );
}

#[test]
fn configured_admin_only_prefixes_replace_the_built_in_prefix() {
    let eng = engine(
        r#"
governance:
  enabled: true
  policies:
    - id: scope_check
      enabled: true
      admin_only_prefixes: ["mcp__internal__"]
"#,
    );
    let call = Call::new("user-scope-config");
    let input = args(json!({}));

    let restricted = tool("mcp__internal__reset");
    let denied = eng
        .evaluate(&call.ctx(&restricted, AccessScope::User, &input))
        .decision;
    assert!(
        matches!(denied, Decision::Deny { .. }),
        "configured prefix must deny for a non-admin, got {denied:?}"
    );

    let former_default = tool("mcp__systemprompt__list_agents");
    let allowed = eng
        .evaluate(&call.ctx(&former_default, AccessScope::User, &input))
        .decision;
    assert!(
        matches!(allowed, Decision::Allow { .. }),
        "the default prefix must no longer be restricted once configured, got {allowed:?}"
    );
}

#[test]
fn every_instantiated_policy_exposes_a_distinct_operator_facing_name_and_description() {
    let eng = engine("governance:\n  enabled: true\n  policies: []\n");

    const BUILTINS: [&str; 4] = ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"];

    let mut seen: Vec<(String, &'static str, &'static str)> = Vec::new();
    for (cfg, policy) in eng.policies() {
        assert_eq!(
            policy.id().as_str(),
            cfg.id.as_str(),
            "a policy's reported id must match the chain entry it was built for"
        );
        if !BUILTINS.contains(&cfg.id.as_str()) {
            continue;
        }
        assert!(
            !policy.name().trim().is_empty(),
            "{} exposes no name",
            cfg.id
        );
        assert!(
            policy.description().len() > 20,
            "{} exposes no usable description",
            cfg.id
        );
        seen.push((cfg.id.clone(), policy.name(), policy.description()));
    }

    assert_eq!(
        seen.len(),
        BUILTINS.len(),
        "every built-in policy must be appended when absent from config, got {seen:?}"
    );
    let mut names: Vec<&str> = seen.iter().map(|(_, n, _)| *n).collect();
    names.sort_unstable();
    let unique = names.len();
    names.dedup();
    assert_eq!(unique, names.len(), "policy names must be distinct");
}

#[test]
fn the_rate_limiter_sweeps_stale_buckets_once_the_key_space_grows() {
    let eng = engine(
        r#"
governance:
  enabled: true
  policies:
    - id: rate_limit
      enabled: true
      requests_per_window: 1
      window_secs: 0
"#,
    );
    let target = tool("read_file");
    let input = args(json!({}));

    // The sweep only runs past the internal bucket threshold; a zero-second
    // window makes every recorded charge immediately stale, so the sweep must
    // reclaim them rather than let the limiter deny on its own history.
    for i in 0..1100 {
        let call = Call::new(&format!("sweep-user-{i}"));
        let decision = eng
            .evaluate(&call.ctx(&target, AccessScope::User, &input))
            .decision;
        assert!(
            matches!(decision, Decision::Allow { .. }),
            "distinct callers must never share a rate-limit bucket (iteration {i}): {decision:?}"
        );
    }

    let repeat = Call::new("sweep-user-repeat");
    for _ in 0..3 {
        let decision = eng
            .evaluate(&repeat.ctx(&target, AccessScope::User, &input))
            .decision;
        assert!(
            matches!(decision, Decision::Allow { .. }),
            "an expired window must not accumulate charges: {decision:?}"
        );
    }
}
