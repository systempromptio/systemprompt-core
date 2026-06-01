use systemprompt_security::authz::{Access, AccessControlConfig, EntityKind, RuleEntry};

fn make_rule(
    entity_id: &str,
    entity_type: EntityKind,
    access: Access,
    roles: Vec<&str>,
) -> RuleEntry {
    RuleEntry {
        entity_type,
        entity_id: entity_id.to_owned(),
        access,
        roles: roles.iter().map(|s| s.to_string()).collect(),
        justification: None,
    }
}

#[test]
fn valid_config_passes() {
    let cfg = AccessControlConfig {
        rules: vec![
            make_rule(
                "claude-3",
                EntityKind::GatewayRoute,
                Access::Allow,
                vec!["user"],
            ),
            make_rule(
                "my-plugin",
                EntityKind::Plugin,
                Access::Deny,
                vec!["contractor"],
            ),
        ],
    };
    assert!(cfg.validate().is_ok());
}

#[test]
fn empty_config_passes() {
    let cfg = AccessControlConfig::default();
    assert!(cfg.validate().is_ok());
}

#[test]
fn empty_entity_id_fails() {
    let cfg = AccessControlConfig {
        rules: vec![make_rule(
            "",
            EntityKind::Plugin,
            Access::Allow,
            vec!["user"],
        )],
    };
    let err = cfg.validate().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("entity_id is empty"), "got: {s}");
}

#[test]
fn whitespace_entity_id_fails() {
    let cfg = AccessControlConfig {
        rules: vec![make_rule(
            "   ",
            EntityKind::Agent,
            Access::Allow,
            vec!["user"],
        )],
    };
    let err = cfg.validate().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("entity_id is empty"), "got: {s}");
}

#[test]
fn empty_roles_fails() {
    let cfg = AccessControlConfig {
        rules: vec![make_rule(
            "my-agent",
            EntityKind::Agent,
            Access::Allow,
            vec![],
        )],
    };
    let err = cfg.validate().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("at least one role"), "got: {s}");
}

#[test]
fn empty_role_string_fails() {
    let cfg = AccessControlConfig {
        rules: vec![make_rule(
            "my-mcp",
            EntityKind::McpServer,
            Access::Deny,
            vec![""],
        )],
    };
    let err = cfg.validate().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("empty role string"), "got: {s}");
}

#[test]
fn multiple_validation_errors_join() {
    let cfg = AccessControlConfig {
        rules: vec![
            make_rule("", EntityKind::Plugin, Access::Allow, vec![]),
            make_rule("good-id", EntityKind::Skill, Access::Allow, vec![]),
        ],
    };
    let err = cfg.validate().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("entity_id is empty"), "got: {s}");
    assert!(s.contains("at least one role"), "got: {s}");
}

#[test]
fn rule_entry_with_justification() {
    let rule = RuleEntry {
        entity_type: EntityKind::Hook,
        entity_id: "my-hook".to_owned(),
        access: Access::Deny,
        roles: vec!["external".to_owned()],
        justification: Some("ITAR restriction".to_owned()),
    };
    let cfg = AccessControlConfig { rules: vec![rule] };
    assert!(cfg.validate().is_ok());
}

#[test]
fn config_serde_roundtrip() {
    let cfg = AccessControlConfig {
        rules: vec![make_rule(
            "marketplace-1",
            EntityKind::Marketplace,
            Access::Allow,
            vec!["user", "admin"],
        )],
    };
    let s = serde_json::to_string(&cfg).unwrap();
    let back: AccessControlConfig = serde_json::from_str(&s).unwrap();
    assert_eq!(back.rules.len(), 1);
    assert_eq!(back.rules[0].entity_id, "marketplace-1");
    assert_eq!(back.rules[0].roles.len(), 2);
}

#[test]
fn rule_entry_deny_access_type() {
    let rule = make_rule(
        "r1",
        EntityKind::GatewayRoute,
        Access::Deny,
        vec!["contractor"],
    );
    assert_eq!(rule.access, Access::Deny);
    assert_eq!(rule.entity_type, EntityKind::GatewayRoute);
}
