#![allow(clippy::all)]

use systemprompt_config::profile_gateway::resolve_override_prompt_includes;
use systemprompt_identifiers::ProviderId;
use systemprompt_models::profile::{
    GatewayConfigSpec, OverrideRuleAction, ProfileError, SystemPromptRule,
};

fn replace_rule(prompt: &str) -> SystemPromptRule {
    SystemPromptRule {
        provider: Some(ProviderId::new("cerebras")),
        model_pattern: Some("claude-*".to_owned()),
        action: OverrideRuleAction::Replace,
        prompt: Some(prompt.to_owned()),
    }
}

fn spec_with(rules: Vec<SystemPromptRule>) -> GatewayConfigSpec {
    GatewayConfigSpec {
        system_prompt_overrides: rules,
        ..Default::default()
    }
}

#[test]
fn include_resolves_to_file_contents() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("prompt.txt"), "you are a terse assistant").unwrap();
    let mut spec = spec_with(vec![replace_rule("!include prompt.txt")]);

    resolve_override_prompt_includes(dir.path(), &mut spec).unwrap();

    assert_eq!(
        spec.system_prompt_overrides[0].prompt.as_deref(),
        Some("you are a terse assistant")
    );
}

#[test]
fn missing_include_file_is_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut spec = spec_with(vec![replace_rule("!include absent.txt")]);

    let err = resolve_override_prompt_includes(dir.path(), &mut spec).unwrap_err();

    assert!(
        matches!(err, ProfileError::ReadFile { .. }),
        "expected ReadFile, got: {err:?}"
    );
}

#[test]
fn inline_prompt_and_strip_rule_pass_through() {
    let dir = tempfile::tempdir().unwrap();
    let strip = SystemPromptRule {
        provider: None,
        model_pattern: None,
        action: OverrideRuleAction::Strip,
        prompt: None,
    };
    let mut spec = spec_with(vec![replace_rule("a literal prompt body"), strip]);

    resolve_override_prompt_includes(dir.path(), &mut spec).unwrap();

    assert_eq!(
        spec.system_prompt_overrides[0].prompt.as_deref(),
        Some("a literal prompt body")
    );
    assert_eq!(spec.system_prompt_overrides[1].prompt, None);
}

#[test]
fn backfill_route_ids_fills_only_blank_ids() {
    let mut spec = GatewayConfigSpec::default();
    spec.routes = vec![route("  "), route("keep-me")];

    let mutated = systemprompt_config::profile_gateway::backfill_route_ids(&mut spec);

    assert!(mutated);
    assert!(!spec.routes[0].id.as_str().trim().is_empty());
    assert_eq!(spec.routes[1].id.as_str(), "keep-me");
}

#[test]
fn backfill_route_ids_no_op_when_all_ids_present() {
    let mut spec = GatewayConfigSpec::default();
    spec.routes = vec![route("route-a")];

    assert!(!systemprompt_config::profile_gateway::backfill_route_ids(
        &mut spec
    ));
    assert_eq!(spec.routes[0].id.as_str(), "route-a");
}

fn route(id: &str) -> systemprompt_models::profile::GatewayRoute {
    systemprompt_models::profile::GatewayRoute {
        id: systemprompt_identifiers::RouteId::new(id),
        model_pattern: "claude-*".to_owned(),
        provider: ProviderId::new("cerebras"),
        upstream_model: None,
        extra_headers: std::collections::HashMap::new(),
        pricing: None,
        when: None,
    }
}
