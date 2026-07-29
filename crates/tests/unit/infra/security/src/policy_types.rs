use std::str::FromStr;

use serde_json::json;
use systemprompt_identifiers::{PolicyId, UserId};
use systemprompt_security::authz::types::{Decision, MatchedBy};
use systemprompt_security::policy::governed::McpToolInput;
use systemprompt_security::policy::types::{
    AccessScope, AgentScope, GovernancePolicy, PolicyContext, RateLimitWindow, SecretLocation,
};

#[test]
fn secret_location_new_stores_fields() {
    let loc = SecretLocation::new("arg", "input.path", "AKIA...[REDACTED]");
    assert_eq!(loc.kind, "arg");
    assert_eq!(loc.path, "input.path");
    assert_eq!(loc.redacted, "AKIA...[REDACTED]");
}

#[test]
fn secret_location_serde_roundtrip() {
    let loc = SecretLocation::new("env", "ENV_SECRET", "ghp_...[REDACTED]");
    let s = serde_json::to_string(&loc).unwrap();
    let back: SecretLocation = serde_json::from_str(&s).unwrap();
    assert_eq!(back, loc);
}

#[test]
fn rate_limit_window_serde_roundtrip() {
    let w = RateLimitWindow {
        name: "per_minute".to_owned(),
        seconds: 60,
        limit: 30,
    };
    let s = serde_json::to_string(&w).unwrap();
    let back: RateLimitWindow = serde_json::from_str(&s).unwrap();
    assert_eq!(back, w);
}

#[test]
fn agent_scope_user_id() {
    let uid = UserId::new("user-123");
    let scope = AgentScope::User {
        user_id: uid.clone(),
    };
    assert_eq!(scope.user_id(), Some(&uid));
    let system = AgentScope::System;
    assert!(system.user_id().is_none());
}

#[test]
fn agent_scope_serde_roundtrip() {
    let uid = UserId::new("u1");
    let scope = AgentScope::User { user_id: uid };
    let s = serde_json::to_string(&scope).unwrap();
    assert!(s.contains("\"kind\":\"user\""), "got: {s}");
    let back: AgentScope = serde_json::from_str(&s).unwrap();
    assert!(back.user_id().is_some());

    let system = AgentScope::System;
    let s2 = serde_json::to_string(&system).unwrap();
    assert!(s2.contains("\"kind\":\"system\""), "got: {s2}");
    let back2: AgentScope = serde_json::from_str(&s2).unwrap();
    assert!(back2.user_id().is_none());
}

#[test]
fn access_scope_as_str_and_display() {
    assert_eq!(AccessScope::Admin.as_str(), "admin");
    assert_eq!(AccessScope::User.as_str(), "user");
    assert_eq!(AccessScope::Unknown.as_str(), "unknown");
    assert_eq!(format!("{}", AccessScope::Admin), "admin");
    assert_eq!(format!("{}", AccessScope::User), "user");
    assert_eq!(format!("{}", AccessScope::Unknown), "unknown");
}

#[test]
fn access_scope_from_str_valid() {
    assert_eq!(AccessScope::from_str("admin").unwrap(), AccessScope::Admin);
    assert_eq!(AccessScope::from_str("user").unwrap(), AccessScope::User);
    assert_eq!(
        AccessScope::from_str("unknown").unwrap(),
        AccessScope::Unknown
    );
    assert_eq!(AccessScope::from_str("").unwrap(), AccessScope::Unknown);
}

#[test]
fn access_scope_from_str_invalid() {
    assert!(AccessScope::from_str("superadmin").is_err());
    assert!(AccessScope::from_str("root").is_err());
}

#[test]
fn access_scope_serde_roundtrip() {
    for scope in [AccessScope::Admin, AccessScope::User, AccessScope::Unknown] {
        let s = serde_json::to_string(&scope).unwrap();
        let back: AccessScope = serde_json::from_str(&s).unwrap();
        assert_eq!(back, scope);
    }
}

#[test]
fn mcp_tool_input_field_extraction() {
    let val = json!({ "path": "/tmp/foo", "content": "hello" });
    let input = McpToolInput::new(val);
    assert_eq!(input.as_str("path"), Some("/tmp/foo"));
    assert_eq!(input.as_path("path"), Some("/tmp/foo"));
    assert_eq!(input.as_str("content"), Some("hello"));
    assert!(input.as_str("missing").is_none());
}

#[test]
fn mcp_tool_input_as_value() {
    let val = json!({ "x": 1 });
    let input = McpToolInput::new(val.clone());
    assert_eq!(input.as_value(), &val);
}

#[test]
fn mcp_tool_input_serde_roundtrip() {
    let val = json!({ "cmd": "ls", "args": ["-la"] });
    let input = McpToolInput::new(val);
    let s = serde_json::to_string(&input).unwrap();
    let back: McpToolInput = serde_json::from_str(&s).unwrap();
    assert_eq!(back.as_str("cmd"), Some("ls"));
}

#[derive(Debug)]
struct AllowPolicy;

impl GovernancePolicy for AllowPolicy {
    fn id(&self) -> PolicyId {
        PolicyId::new("test-allow")
    }
    fn name(&self) -> &'static str {
        "allow-all"
    }
    fn description(&self) -> &'static str {
        "always allows"
    }
    fn evaluate(&self, _ctx: &PolicyContext<'_>) -> Decision {
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        }
    }
}

#[test]
fn governance_policy_metadata() {
    let p = AllowPolicy;
    assert_eq!(p.name(), "allow-all");
    assert_eq!(p.description(), "always allows");
    assert_eq!(p.id().as_str(), "test-allow");
}
