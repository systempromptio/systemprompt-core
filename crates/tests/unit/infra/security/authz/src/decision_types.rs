use std::borrow::Cow;

use serde_json;
use systemprompt_identifiers::{AgentId, McpToolName, RouteId, SecretPatternId, UserId};
use systemprompt_security::authz::{
    AuthzDecision, Decision, DecisionTag, DenyReason, EntityRef, MatchedBy,
};
use systemprompt_security::policy::types::{AccessScope, RateLimitWindow, SecretLocation};

#[test]
fn decision_tag_as_str() {
    assert_eq!(DecisionTag::Allow.as_str(), "allow");
    assert_eq!(DecisionTag::Deny.as_str(), "deny");
}

#[test]
fn decision_tag_display() {
    assert_eq!(format!("{}", DecisionTag::Allow), "allow");
    assert_eq!(format!("{}", DecisionTag::Deny), "deny");
}

#[test]
fn decision_tag_from_authz_decision() {
    let allow: AuthzDecision = AuthzDecision::Allow;
    let tag = DecisionTag::from(&allow);
    assert_eq!(tag, DecisionTag::Allow);

    let deny = AuthzDecision::Deny {
        reason: DenyReason::HookUnavailable {
            policy: "test".to_owned(),
        },
        policy: "test".to_owned(),
    };
    let tag2 = DecisionTag::from(&deny);
    assert_eq!(tag2, DecisionTag::Deny);
}

#[test]
fn decision_allow_tag() {
    let d = Decision::Allow {
        matched_by: MatchedBy::UserAllow,
    };
    assert_eq!(d.tag(), DecisionTag::Allow);
}

#[test]
fn decision_deny_tag() {
    let d = Decision::Deny {
        reason: DenyReason::HookUnavailable {
            policy: "p".to_owned(),
        },
    };
    assert_eq!(d.tag(), DecisionTag::Deny);
}

#[test]
fn decision_allow_serde_roundtrip() {
    for mb in [
        MatchedBy::UserAllow,
        MatchedBy::DefaultIncluded,
        MatchedBy::RoleAllow {
            role: "admin".to_owned(),
        },
    ] {
        let d = Decision::Allow { matched_by: mb };
        let s = serde_json::to_string(&d).unwrap();
        let back: Decision = serde_json::from_str(&s).unwrap();
        assert!(matches!(back, Decision::Allow { .. }), "got {back:?}");
    }
}

#[test]
fn deny_reason_user_deny_display() {
    let entity = EntityRef::GatewayRoute(RouteId::new("r1"));
    let user_id = UserId::new("alice");
    let r = DenyReason::UserDeny {
        entity: entity.clone(),
        user_id: user_id.clone(),
        justification: None,
    };
    let s = r.to_string();
    assert!(s.contains("alice"), "got: {s}");
}

#[test]
fn deny_reason_role_deny_display() {
    let entity = EntityRef::Agent(AgentId::new("my-agent"));
    let r = DenyReason::RoleDeny {
        entity,
        role: "contractor".to_owned(),
        justification: Some("no access".to_owned()),
    };
    let s = r.to_string();
    assert!(s.contains("contractor"), "got: {s}");
}

#[test]
fn deny_reason_not_assigned_display() {
    let entity = EntityRef::GatewayRoute(RouteId::new("route-1"));
    let user_id = UserId::new("bob");
    let r = DenyReason::NotAssigned {
        entity,
        user_id,
        roles: vec!["eng".to_owned()],
    };
    let s = r.to_string();
    assert!(s.contains("bob"), "got: {s}");
}

#[test]
fn deny_reason_unknown_entity_display() {
    let entity = EntityRef::GatewayRoute(RouteId::new("unknown-route"));
    let r = DenyReason::UnknownEntity { entity };
    let s = r.to_string();
    assert!(s.contains("unknown"), "got: {s}");
}

#[test]
fn deny_reason_hook_unavailable_display() {
    let r = DenyReason::HookUnavailable {
        policy: "authz_rule_based".to_owned(),
    };
    let s = r.to_string();
    assert!(s.contains("authz_rule_based"), "got: {s}");
}

#[test]
fn deny_reason_policy_violation_display() {
    let r = DenyReason::PolicyViolation {
        policy: "itar".to_owned(),
        detail: Cow::Borrowed("jurisdiction blocked"),
    };
    let s = r.to_string();
    assert!(s.contains("jurisdiction blocked"), "got: {s}");
}

#[test]
fn deny_reason_secret_leak_display() {
    let r = DenyReason::SecretLeak {
        pattern_id: SecretPatternId::new("aws-key"),
        pattern_name: Cow::Borrowed("AWS Secret Key"),
        location: SecretLocation::new("arg", "input.key"),
    };
    let s = r.to_string();
    assert!(s.contains("AWS Secret Key"), "got: {s}");
}

#[test]
fn deny_reason_scope_violation_display() {
    let r = DenyReason::ScopeViolation {
        tool: McpToolName::new("exec"),
        required: AccessScope::Admin,
    };
    let s = r.to_string();
    assert!(s.contains("exec"), "got: {s}");
    assert!(s.contains("admin"), "got: {s}");
}

#[test]
fn deny_reason_tool_blocked_display() {
    let r = DenyReason::ToolBlocked {
        tool: McpToolName::new("rm"),
        list_id: "blocklist-1".to_owned(),
    };
    let s = r.to_string();
    assert!(s.contains("rm"), "got: {s}");
    assert!(s.contains("blocklist-1"), "got: {s}");
}

#[test]
fn deny_reason_rate_limit_exceeded_display() {
    let r = DenyReason::RateLimitExceeded {
        window: RateLimitWindow {
            name: "per_minute".to_owned(),
            seconds: 60,
            limit: 30,
        },
        retry_after_ms: 5000,
    };
    let s = r.to_string();
    assert!(s.contains("5000"), "got: {s}");
}

#[test]
fn deny_reason_serde_roundtrip_hook_unavailable() {
    let r = DenyReason::HookUnavailable {
        policy: "authz_default_deny".to_owned(),
    };
    let s = serde_json::to_string(&r).unwrap();
    let back: DenyReason = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, DenyReason::HookUnavailable { .. }));
}

#[test]
fn matched_by_serde_user_allow() {
    let mb = MatchedBy::UserAllow;
    let s = serde_json::to_string(&mb).unwrap();
    assert!(s.contains("user_allow"), "got: {s}");
    let back: MatchedBy = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, MatchedBy::UserAllow));
}

#[test]
fn matched_by_serde_role_allow() {
    let mb = MatchedBy::RoleAllow {
        role: "ops".to_owned(),
    };
    let s = serde_json::to_string(&mb).unwrap();
    assert!(s.contains("ops"), "got: {s}");
    let back: MatchedBy = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, MatchedBy::RoleAllow { role } if role == "ops"));
}

#[test]
fn decision_tag_serde_roundtrip() {
    for tag in [DecisionTag::Allow, DecisionTag::Deny] {
        let s = serde_json::to_string(&tag).unwrap();
        let back: DecisionTag = serde_json::from_str(&s).unwrap();
        assert_eq!(back, tag);
    }
}
