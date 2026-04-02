use systemprompt_models::validators::{
    AgentConfigValidator, AiConfigValidator, ContentConfigValidator, McpConfigValidator,
    RateLimitsConfigValidator, SkillConfigValidator, WebConfigValidator,
};
use systemprompt_traits::DomainConfig;

// ============================================================================
// AgentConfigValidator Tests
// ============================================================================

#[test]
fn agent_validator_new_creates_instance() {
    let validator = AgentConfigValidator::new();
    assert_eq!(validator.domain_id(), "agents");
}

#[test]
fn agent_validator_domain_id() {
    let validator = AgentConfigValidator::new();
    assert_eq!(validator.domain_id(), "agents");
}

#[test]
fn agent_validator_priority() {
    let validator = AgentConfigValidator::new();
    assert_eq!(validator.priority(), 30);
}

#[test]
fn agent_validator_validate_fails_when_not_loaded() {
    let validator = AgentConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_err());
}

#[test]
fn agent_validator_no_dependencies() {
    let validator = AgentConfigValidator::new();
    assert!(validator.dependencies().is_empty());
}

// ============================================================================
// AiConfigValidator Tests
// ============================================================================

#[test]
fn ai_validator_new_creates_instance() {
    let validator = AiConfigValidator::new();
    assert_eq!(validator.domain_id(), "ai");
}

#[test]
fn ai_validator_domain_id() {
    let validator = AiConfigValidator::new();
    assert_eq!(validator.domain_id(), "ai");
}

#[test]
fn ai_validator_priority() {
    let validator = AiConfigValidator::new();
    assert_eq!(validator.priority(), 50);
}

#[test]
fn ai_validator_depends_on_mcp() {
    let validator = AiConfigValidator::new();
    let deps = validator.dependencies();
    assert!(deps.contains(&"mcp"));
}

#[test]
fn ai_validator_validate_fails_when_not_loaded() {
    let validator = AiConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_err());
}

// ============================================================================
// ContentConfigValidator Tests
// ============================================================================

#[test]
fn content_validator_new_creates_instance() {
    let validator = ContentConfigValidator::new();
    assert_eq!(validator.domain_id(), "content");
}

#[test]
fn content_validator_domain_id() {
    let validator = ContentConfigValidator::new();
    assert_eq!(validator.domain_id(), "content");
}

#[test]
fn content_validator_priority() {
    let validator = ContentConfigValidator::new();
    assert_eq!(validator.priority(), 20);
}

#[test]
fn content_validator_no_dependencies() {
    let validator = ContentConfigValidator::new();
    assert!(validator.dependencies().is_empty());
}

#[test]
fn content_validator_validate_succeeds_when_not_loaded_with_no_content() {
    let validator = ContentConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_ok());
    let report = result.unwrap();
    assert!(!report.has_errors());
}

// ============================================================================
// McpConfigValidator Tests
// ============================================================================

#[test]
fn mcp_validator_new_creates_instance() {
    let validator = McpConfigValidator::new();
    assert_eq!(validator.domain_id(), "mcp");
}

#[test]
fn mcp_validator_domain_id() {
    let validator = McpConfigValidator::new();
    assert_eq!(validator.domain_id(), "mcp");
}

#[test]
fn mcp_validator_priority() {
    let validator = McpConfigValidator::new();
    assert_eq!(validator.priority(), 40);
}

#[test]
fn mcp_validator_depends_on_agents() {
    let validator = McpConfigValidator::new();
    let deps = validator.dependencies();
    assert!(deps.contains(&"agents"));
}

#[test]
fn mcp_validator_validate_returns_ok_when_no_config() {
    let validator = McpConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_ok());
}

// ============================================================================
// SkillConfigValidator Tests
// ============================================================================

#[test]
fn skill_validator_new_creates_instance() {
    let validator = SkillConfigValidator::new();
    assert_eq!(validator.domain_id(), "skills");
}

#[test]
fn skill_validator_domain_id() {
    let validator = SkillConfigValidator::new();
    assert_eq!(validator.domain_id(), "skills");
}

#[test]
fn skill_validator_priority() {
    let validator = SkillConfigValidator::new();
    assert_eq!(validator.priority(), 25);
}

#[test]
fn skill_validator_no_dependencies() {
    let validator = SkillConfigValidator::new();
    assert!(validator.dependencies().is_empty());
}

#[test]
fn skill_validator_validate_fails_when_not_loaded() {
    let validator = SkillConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_err());
}

// ============================================================================
// RateLimitsConfigValidator Tests
// ============================================================================

#[test]
fn rate_limits_validator_new_creates_instance() {
    let validator = RateLimitsConfigValidator::new();
    assert_eq!(validator.domain_id(), "rate_limits");
}

#[test]
fn rate_limits_validator_domain_id() {
    let validator = RateLimitsConfigValidator::new();
    assert_eq!(validator.domain_id(), "rate_limits");
}

#[test]
fn rate_limits_validator_priority() {
    let validator = RateLimitsConfigValidator::new();
    assert_eq!(validator.priority(), 10);
}

#[test]
fn rate_limits_validator_no_dependencies() {
    let validator = RateLimitsConfigValidator::new();
    assert!(validator.dependencies().is_empty());
}

#[test]
fn rate_limits_validator_validate_fails_when_not_loaded() {
    let validator = RateLimitsConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_err());
}

// ============================================================================
// WebConfigValidator Tests
// ============================================================================

#[test]
fn web_validator_new_creates_instance() {
    let validator = WebConfigValidator::new();
    assert_eq!(validator.domain_id(), "web");
}

#[test]
fn web_validator_domain_id() {
    let validator = WebConfigValidator::new();
    assert_eq!(validator.domain_id(), "web");
}

#[test]
fn web_validator_priority() {
    let validator = WebConfigValidator::new();
    assert_eq!(validator.priority(), 10);
}

#[test]
fn web_validator_no_dependencies() {
    let validator = WebConfigValidator::new();
    assert!(validator.dependencies().is_empty());
}

#[test]
fn web_validator_validate_returns_ok_when_no_config() {
    let validator = WebConfigValidator::new();
    let result = validator.validate();
    assert!(result.is_ok());
}

// ============================================================================
// Validator Priority Ordering Tests
// ============================================================================

#[test]
fn rate_limits_and_web_have_lowest_priority() {
    let rate = RateLimitsConfigValidator::new();
    let web = WebConfigValidator::new();
    assert_eq!(rate.priority(), web.priority());
    assert_eq!(rate.priority(), 10);
}

#[test]
fn content_priority_lower_than_agents() {
    let content = ContentConfigValidator::new();
    let agents = AgentConfigValidator::new();
    assert!(content.priority() < agents.priority());
}

#[test]
fn skills_priority_between_content_and_agents() {
    let skills = SkillConfigValidator::new();
    let content = ContentConfigValidator::new();
    let agents = AgentConfigValidator::new();
    assert!(skills.priority() > content.priority());
    assert!(skills.priority() < agents.priority());
}

#[test]
fn mcp_priority_higher_than_agents() {
    let mcp = McpConfigValidator::new();
    let agents = AgentConfigValidator::new();
    assert!(mcp.priority() > agents.priority());
}

#[test]
fn ai_has_highest_priority() {
    let ai = AiConfigValidator::new();
    let mcp = McpConfigValidator::new();
    let agents = AgentConfigValidator::new();
    assert!(ai.priority() > mcp.priority());
    assert!(ai.priority() > agents.priority());
}

// ============================================================================
// ValidationConfigProvider Tests
// ============================================================================

#[test]
fn validation_config_provider_web_config_raw_deserializes_empty() {
    use systemprompt_models::validators::WebConfigRaw;
    let raw: WebConfigRaw = serde_json::from_str("{}").unwrap();
    assert!(raw.site_name.is_none());
    assert!(raw.base_url.is_none());
    assert!(raw.theme.is_none());
    assert!(raw.branding.is_none());
    assert!(raw.paths.is_none());
}

#[test]
fn validation_config_provider_web_config_raw_with_fields() {
    use systemprompt_models::validators::WebConfigRaw;
    let json = r#"{"site_name": "MySite", "base_url": "https://example.com"}"#;
    let raw: WebConfigRaw = serde_json::from_str(json).unwrap();
    assert_eq!(raw.site_name, Some("MySite".to_string()));
    assert_eq!(raw.base_url, Some("https://example.com".to_string()));
}

#[test]
fn validation_config_provider_web_metadata_raw_deserializes_empty() {
    use systemprompt_models::validators::WebMetadataRaw;
    let raw: WebMetadataRaw = serde_json::from_str("{}").unwrap();
    assert!(raw.title.is_none());
    assert!(raw.description.is_none());
}

#[test]
fn validation_config_provider_web_metadata_raw_with_fields() {
    use systemprompt_models::validators::WebMetadataRaw;
    let json = r#"{"title": "My Site", "description": "A description"}"#;
    let raw: WebMetadataRaw = serde_json::from_str(json).unwrap();
    assert_eq!(raw.title, Some("My Site".to_string()));
    assert_eq!(raw.description, Some("A description".to_string()));
}
