use systemprompt_ai::{OverrideAction, OverrideContext, OverrideEngine, OverrideSource};
use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_models::profile::{OverrideRuleAction, SystemPromptRule};

fn ctx(provider: &str, model: &str, system: Option<&str>) -> OverrideContext {
    OverrideContext::builder(ProviderId::new(provider), ModelId::new(model))
        .current_system(system.map(ToOwned::to_owned))
        .build()
}

fn replace_rule(provider: Option<&str>, pattern: Option<&str>, prompt: &str) -> SystemPromptRule {
    SystemPromptRule {
        provider: provider.map(ProviderId::new),
        model_pattern: pattern.map(ToOwned::to_owned),
        action: OverrideRuleAction::Replace,
        prompt: Some(prompt.to_owned()),
    }
}

#[tokio::test]
async fn replace_rule_substitutes_prompt() {
    let rules = vec![replace_rule(Some("cerebras"), Some("claude-*"), "light")];
    let resolution = OverrideEngine::global()
        .resolve(&rules, &ctx("cerebras", "claude-opus-4-8", Some("huge")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Replace("light".to_owned()));
    assert_eq!(resolution.source, Some(OverrideSource::Config));
    assert_eq!(resolution.audit_descriptor().as_deref(), Some("config:replace"));
}

#[tokio::test]
async fn strip_rule_removes_prompt() {
    let rules = vec![SystemPromptRule {
        provider: Some(ProviderId::new("cerebras")),
        model_pattern: None,
        action: OverrideRuleAction::Strip,
        prompt: None,
    }];
    let resolution = OverrideEngine::global()
        .resolve(&rules, &ctx("cerebras", "claude-3-7-sonnet", Some("huge")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Strip);
    assert_eq!(resolution.audit_descriptor().as_deref(), Some("config:strip"));
}

#[tokio::test]
async fn non_matching_provider_passes_through() {
    let rules = vec![replace_rule(Some("cerebras"), Some("claude-*"), "light")];
    let resolution = OverrideEngine::global()
        .resolve(&rules, &ctx("openai", "claude-opus-4-8", Some("keep")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Passthrough);
    assert_eq!(resolution.source, None);
    assert_eq!(resolution.audit_descriptor(), None);
}

#[tokio::test]
async fn first_matching_rule_wins() {
    let rules = vec![
        replace_rule(None, Some("claude-*"), "first"),
        replace_rule(Some("cerebras"), Some("claude-*"), "second"),
    ];
    let resolution = OverrideEngine::global()
        .resolve(&rules, &ctx("cerebras", "claude-opus-4-8", None))
        .await;
    assert_eq!(resolution.action, OverrideAction::Replace("first".to_owned()));
}

#[tokio::test]
async fn empty_rules_pass_through() {
    let resolution = OverrideEngine::global()
        .resolve(&[], &ctx("cerebras", "claude-opus-4-8", Some("keep")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Passthrough);
    assert_eq!(resolution.audit_descriptor(), None);
}
