use systemprompt_agent::services::registry::skills::{extract_description, load_skill_from_disk};
use systemprompt_agent::services::registry::load_agent_skills_from_dir;
use systemprompt_identifiers::SkillId;
use systemprompt_models::services::{
    AgentCardConfig, AgentConfig, AgentMetadataConfig, CapabilitiesConfig, OAuthConfig,
};
use tempfile::TempDir;

#[test]
fn test_extract_description_with_valid_frontmatter() {
    let content = "---\ndescription: A helpful skill\n---\nBody content here";
    let result = extract_description(content);
    assert_eq!(result, Some("A helpful skill".to_string()));
}

#[test]
fn test_extract_description_no_frontmatter() {
    let content = "Just regular markdown content";
    let result = extract_description(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_description_empty_string() {
    let result = extract_description("");
    assert!(result.is_none());
}

#[test]
fn test_extract_description_frontmatter_without_description_field() {
    let content = "---\ntitle: My Skill\ntags: [a, b]\n---\nBody";
    let result = extract_description(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_description_unclosed_frontmatter() {
    let content = "---\ndescription: test\nno closing delimiter";
    let result = extract_description(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_description_empty_frontmatter() {
    let content = "---\n---\nBody text";
    let result = extract_description(content);
    assert!(result.is_none());
}

#[test]
fn test_extract_description_multiline_frontmatter() {
    let content =
        "---\ntitle: Test\ndescription: Multi word description here\nversion: 1.0\n---\nContent";
    let result = extract_description(content);
    assert_eq!(result, Some("Multi word description here".to_string()));
}

#[test]
fn test_extract_description_starts_with_dashes_but_not_frontmatter() {
    let content = "---not yaml---\nstuff";
    let result = extract_description(content);
    assert!(result.is_none());
}

#[test]
fn test_load_skill_from_disk_with_skill_md_only() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("my-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "This is a skill description").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("my-skill"));
    let skill = result.expect("expected success");

    assert_eq!(skill.id.as_str(), "my-skill");
    assert_eq!(skill.name, "my-skill");
    assert_eq!(skill.description, "my-skill skill");
    assert!(skill.tags.is_empty());
    assert!(skill.examples.is_none());
    assert!(skill.input_modes.is_none());
    assert!(skill.output_modes.is_none());
}

#[test]
fn test_load_skill_from_disk_with_config_yaml() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("writer");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "Skill content").unwrap();
    std::fs::write(
        skill_dir.join("config.yaml"),
        r#"
name: Blog Writer
description: Writes blog posts
tags:
  - writing
  - content
examples:
  - Write a blog post about Rust
input_modes:
  - text/plain
output_modes:
  - text/markdown
"#,
    )
    .unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("writer"));
    let skill = result.expect("expected success");

    assert_eq!(skill.id.as_str(), "writer");
    assert_eq!(skill.name, "Blog Writer");
    assert_eq!(skill.description, "Writes blog posts");
    assert_eq!(skill.tags, vec!["writing", "content"]);
    assert_eq!(
        skill.examples,
        Some(vec!["Write a blog post about Rust".to_string()])
    );
    assert_eq!(skill.input_modes, Some(vec!["text/plain".to_string()]));
    assert_eq!(skill.output_modes, Some(vec!["text/markdown".to_string()]));
}

#[test]
fn test_load_skill_from_disk_missing_skill_md() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("no-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("no-skill"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("SKILL.md"));
}

#[test]
fn test_load_skill_from_disk_missing_directory() {
    let dir = TempDir::new().unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("nonexistent"));
    assert!(result.is_err());
}

#[test]
fn test_load_skill_from_disk_config_overrides_frontmatter_description() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("override-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Frontmatter description\n---\nBody",
    )
    .unwrap();
    std::fs::write(
        skill_dir.join("config.yaml"),
        "description: Config description\n",
    )
    .unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("override-skill"));
    let skill = result.expect("expected success");

    assert_eq!(skill.description, "Config description");
}

#[test]
fn test_load_skill_from_disk_frontmatter_description_used_when_no_config_description() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("frontmatter-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: From frontmatter\n---\nBody",
    )
    .unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "tags:\n  - test\n").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("frontmatter-skill"));
    let skill = result.expect("expected success");

    assert_eq!(skill.description, "From frontmatter");
}

#[test]
fn test_load_skill_from_disk_fallback_description() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("fallback-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "No frontmatter content").unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "tags: []\n").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("fallback-skill"));
    let skill = result.expect("expected success");

    assert_eq!(skill.description, "fallback-skill skill");
}

#[test]
fn test_load_skill_from_disk_config_name_override() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("name-test");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "name: Custom Name\n").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("name-test"));
    let skill = result.expect("expected success");

    assert_eq!(skill.name, "Custom Name");
}

#[test]
fn test_load_skill_from_disk_name_defaults_to_skill_id() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("default-name");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("default-name"));
    let skill = result.expect("expected success");

    assert_eq!(skill.name, "default-name");
}

#[test]
fn test_load_skill_from_disk_invalid_config_yaml() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("bad-config");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "{{{{invalid yaml").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("bad-config"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("config.yaml"));
}

#[test]
fn test_load_skill_from_disk_security_is_none() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("sec-test");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("sec-test"));
    let skill = result.expect("expected success");

    assert!(skill.security.is_none());
}

#[test]
fn test_load_skill_from_disk_empty_tags_in_config() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("empty-tags");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "tags: []\n").unwrap();

    let result = load_skill_from_disk(dir.path(), &SkillId::new("empty-tags"));
    let skill = result.expect("expected success");

    assert!(skill.tags.is_empty());
}

fn agent_card_config_empty() -> AgentCardConfig {
    AgentCardConfig {
        protocol_version: "1.0".to_owned(),
        name: None,
        display_name: "A".to_owned(),
        description: "d".to_owned(),
        version: "1.0.0".to_owned(),
        preferred_transport: "JSONRPC".to_owned(),
        icon_url: None,
        documentation_url: None,
        provider: None,
        capabilities: CapabilitiesConfig::default(),
        default_input_modes: vec!["text/plain".to_owned()],
        default_output_modes: vec!["text/plain".to_owned()],
        security_schemes: None,
        security: None,
        skills: Vec::new(),
        supports_authenticated_extended_card: false,
    }
}

fn agent_with_metadata_skills(skills: Vec<String>) -> AgentConfig {
    AgentConfig {
        name: "demo".to_owned(),
        port: 9100,
        endpoint: String::new(),
        tags: Vec::new(),
        enabled: true,
        dev_only: false,
        is_primary: false,
        default: false,
        card: agent_card_config_empty(),
        metadata: AgentMetadataConfig {
            skills,
            ..AgentMetadataConfig::default()
        },
        oauth: OAuthConfig::default(),
    }
}

#[test]
fn a2a_card_skills_are_joined_from_metadata_skills_against_disk_catalog() {
    // Regression: agent declares `metadata.skills: [example_web_search]` and
    // leaves `card.skills` empty. The A2A card must populate
    // `card.skills` by reading the skill catalog under `skills_dir`.
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("example_web_search");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: Search the public web for an answer.\n---\nbody",
    )
    .unwrap();
    std::fs::write(
        skill_dir.join("config.yaml"),
        "name: Example Web Search\ntags:\n  - search\n",
    )
    .unwrap();

    let agent = agent_with_metadata_skills(vec!["example_web_search".to_owned()]);
    let resolved = load_agent_skills_from_dir(&agent, dir.path());

    assert_eq!(resolved.len(), 1, "expected one resolved skill, got {resolved:?}");
    let skill = &resolved[0];
    assert_eq!(skill.id, "example_web_search");
    assert_eq!(skill.name, "Example Web Search");
    assert_eq!(skill.description, "Search the public web for an answer.");
    assert_eq!(skill.tags, vec!["search".to_owned()]);
}

#[test]
fn a2a_card_skills_drop_unresolvable_metadata_ids_silently() {
    // Skills listed in metadata.skills but missing from the on-disk catalog
    // must be skipped, not crash the card assembly.
    let dir = TempDir::new().unwrap();
    let agent =
        agent_with_metadata_skills(vec!["does_not_exist_on_disk".to_owned()]);

    let resolved = load_agent_skills_from_dir(&agent, dir.path());
    assert!(resolved.is_empty(), "expected empty, got {resolved:?}");
}

#[test]
fn a2a_card_skills_ignore_authored_card_skills() {
    // Even if YAML authored card.skills (now deprecated), the A2A loader must
    // ignore them — only metadata.skills drives the join.
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("foo");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();
    std::fs::write(skill_dir.join("config.yaml"), "name: Foo\n").unwrap();

    let mut agent = agent_with_metadata_skills(vec!["foo".to_owned()]);
    // Author a phantom card.skills entry referencing a skill that does NOT
    // exist on disk. The loader must NOT surface it.
    agent.card.skills = vec![systemprompt_models::services::AgentSkillConfig {
        id: SkillId::new("phantom_bar"),
        name: "Phantom Bar".to_owned(),
        description: "ignored".to_owned(),
        tags: Vec::new(),
        examples: None,
        input_modes: None,
        output_modes: None,
        security: None,
    }];

    let resolved = load_agent_skills_from_dir(&agent, dir.path());
    let ids: Vec<&str> = resolved.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["foo"]);
    assert!(
        !ids.contains(&"phantom_bar"),
        "authored card.skills must be ignored"
    );
}
