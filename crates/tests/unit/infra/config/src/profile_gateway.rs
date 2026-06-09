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
