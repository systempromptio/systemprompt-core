use async_trait::async_trait;
use systemprompt_ai::{
    OverrideAction, OverrideContext, OverrideEngine, OverrideError, OverrideResolution,
    OverrideSource, SystemPromptOverride, register_system_prompt_override,
};
use systemprompt_identifiers::{ModelId, ProviderId};
use systemprompt_models::profile::{OverrideRuleAction, SystemPromptRule};

// Extensions registered here are scoped to sentinel provider names so every
// other test in this binary still sees pass-through behaviour from the global
// engine.
struct ReplacingOverride;

#[async_trait]
impl SystemPromptOverride for ReplacingOverride {
    fn name(&self) -> &'static str {
        "test-replacer"
    }

    async fn evaluate(&self, ctx: &OverrideContext) -> Result<OverrideAction, OverrideError> {
        if ctx.provider().as_str() == "ext-test-provider" {
            Ok(OverrideAction::Replace("from-extension".to_owned()))
        } else {
            Ok(OverrideAction::Passthrough)
        }
    }
}

struct ErroringOverride;

#[async_trait]
impl SystemPromptOverride for ErroringOverride {
    fn name(&self) -> &'static str {
        "test-error"
    }

    async fn evaluate(&self, ctx: &OverrideContext) -> Result<OverrideAction, OverrideError> {
        if ctx.provider().as_str() == "ext-err-provider" {
            Err(OverrideError::Failed {
                name: "test-error",
                message: "boom".to_owned(),
            })
        } else {
            Ok(OverrideAction::Passthrough)
        }
    }
}

const fn make_replacer() -> ReplacingOverride {
    ReplacingOverride
}

const fn make_erroring() -> ErroringOverride {
    ErroringOverride
}

register_system_prompt_override!(make_replacer, name = "test-replacer");
register_system_prompt_override!(make_erroring, name = "test-error");

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
    assert_eq!(
        resolution.action,
        OverrideAction::Replace("light".to_owned())
    );
    assert_eq!(resolution.source, Some(OverrideSource::Config));
    assert_eq!(
        resolution.audit_descriptor().as_deref(),
        Some("config:replace")
    );
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
    assert_eq!(
        resolution.audit_descriptor().as_deref(),
        Some("config:strip")
    );
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
    assert_eq!(
        resolution.action,
        OverrideAction::Replace("first".to_owned())
    );
}

#[tokio::test]
async fn empty_rules_pass_through() {
    let resolution = OverrideEngine::global()
        .resolve(&[], &ctx("cerebras", "claude-opus-4-8", Some("keep")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Passthrough);
    assert_eq!(resolution.audit_descriptor(), None);
}

#[tokio::test]
async fn registered_extension_replaces_when_no_config_rule_matches() {
    let engine = OverrideEngine::global();
    assert!(engine.has_extensions());
    let resolution = engine
        .resolve(&[], &ctx("ext-test-provider", "any-model", Some("orig")))
        .await;
    assert_eq!(
        resolution.action,
        OverrideAction::Replace("from-extension".to_owned())
    );
    assert_eq!(
        resolution.source,
        Some(OverrideSource::Extension("test-replacer"))
    );
    assert_eq!(
        resolution.audit_descriptor().as_deref(),
        Some("extension:test-replacer:replace")
    );
}

#[tokio::test]
async fn config_rule_wins_over_registered_extension() {
    let rules = vec![replace_rule(Some("ext-test-provider"), None, "from-config")];
    let resolution = OverrideEngine::global()
        .resolve(&rules, &ctx("ext-test-provider", "any-model", None))
        .await;
    assert_eq!(
        resolution.action,
        OverrideAction::Replace("from-config".to_owned())
    );
    assert_eq!(resolution.source, Some(OverrideSource::Config));
}

#[tokio::test]
async fn erroring_extension_degrades_to_passthrough() {
    let resolution = OverrideEngine::global()
        .resolve(&[], &ctx("ext-err-provider", "any-model", Some("keep")))
        .await;
    assert_eq!(resolution.action, OverrideAction::Passthrough);
    assert_eq!(resolution.source, None);
}

#[test]
fn builder_defaults_upstream_model_to_requested() {
    let built = OverrideContext::builder(ProviderId::new("p"), ModelId::new("requested")).build();
    assert_eq!(built.upstream_model().as_str(), "requested");
    assert_eq!(built.requested_model().as_str(), "requested");
    assert_eq!(built.current_system(), None);
}

#[test]
fn builder_keeps_explicit_upstream_model() {
    let built = OverrideContext::builder(ProviderId::new("p"), ModelId::new("requested"))
        .upstream_model(ModelId::new("actual"))
        .current_system(Some("sys".to_owned()))
        .build();
    assert_eq!(built.upstream_model().as_str(), "actual");
    assert_eq!(built.current_system(), Some("sys"));
}

#[test]
fn passthrough_resolution_has_no_descriptor_even_with_source() {
    assert_eq!(OverrideResolution::passthrough().audit_descriptor(), None);
    let sourced = OverrideResolution {
        action: OverrideAction::Strip,
        source: Some(OverrideSource::Extension("x")),
    };
    assert_eq!(
        sourced.audit_descriptor().as_deref(),
        Some("extension:x:strip")
    );
    let unsourced = OverrideResolution {
        action: OverrideAction::Replace("p".to_owned()),
        source: None,
    };
    assert_eq!(unsourced.audit_descriptor(), None);
}
