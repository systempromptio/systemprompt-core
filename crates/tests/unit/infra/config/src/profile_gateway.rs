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

#[test]
fn load_profile_with_catalog_resolves_gateway_section() {
    let fx = crate::fixture::write_tree(crate::fixture::FILE_SECRETS, None);
    std::fs::write(
        fx.tmp.path().join("override_prompt.txt"),
        "terse gateway prompt",
    )
    .unwrap();
    let gateway_yaml = r#"providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: https://api.anthropic.com/v1
    api_key_secret: anthropic
    models:
      - id: claude-sonnet-4-5
gateway:
  enabled: true
  default_provider: anthropic
  routes:
    - model_pattern: 'claude-*'
      provider: anthropic
  system_prompt_overrides:
    - action: replace
      prompt: '!include override_prompt.txt'
"#;
    let mut yaml = std::fs::read_to_string(&fx.profile_path).unwrap();
    yaml.push_str(gateway_yaml);
    std::fs::write(&fx.profile_path, yaml).unwrap();

    let profile = systemprompt_config::load_profile_with_catalog(&fx.profile_path).unwrap();

    let gateway = profile.gateway.as_ref().unwrap().resolved().unwrap();
    assert!(gateway.enabled);
    assert_eq!(gateway.routes.len(), 1);
    assert!(!gateway.routes[0].id.as_str().trim().is_empty());
    assert_eq!(gateway.routes[0].provider.as_str(), "anthropic");
    assert_eq!(
        gateway.system_prompt_overrides[0].prompt.as_deref(),
        Some("terse gateway prompt")
    );
}

#[test]
fn load_profile_without_gateway_returns_profile_unchanged() {
    let fx = crate::fixture::write_tree(crate::fixture::FILE_SECRETS, None);

    let profile = systemprompt_config::load_profile_with_catalog(&fx.profile_path).unwrap();

    assert!(profile.gateway.is_none());
    assert_eq!(profile.name, "config_fixture");
}

#[test]
fn load_profile_with_catalog_missing_file_errors() {
    let err =
        systemprompt_config::load_profile_with_catalog(std::path::Path::new("/absent/p.yaml"))
            .unwrap_err();
    assert!(matches!(err, ProfileError::ReadFile { .. }));
}
