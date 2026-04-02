use systemprompt_agent::services::registry::skills::{extract_description, load_skill_from_disk};
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
    let content = "---\ntitle: Test\ndescription: Multi word description here\nversion: 1.0\n---\nContent";
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

    let result = load_skill_from_disk(dir.path(), "my-skill");
    let skill = result.expect("expected success");

    assert_eq!(skill.id, "my-skill");
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

    let result = load_skill_from_disk(dir.path(), "writer");
    let skill = result.expect("expected success");

    assert_eq!(skill.id, "writer");
    assert_eq!(skill.name, "Blog Writer");
    assert_eq!(skill.description, "Writes blog posts");
    assert_eq!(skill.tags, vec!["writing", "content"]);
    assert_eq!(skill.examples, Some(vec!["Write a blog post about Rust".to_string()]));
    assert_eq!(skill.input_modes, Some(vec!["text/plain".to_string()]));
    assert_eq!(skill.output_modes, Some(vec!["text/markdown".to_string()]));
}

#[test]
fn test_load_skill_from_disk_missing_skill_md() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("no-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let result = load_skill_from_disk(dir.path(), "no-skill");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("SKILL.md"));
}

#[test]
fn test_load_skill_from_disk_missing_directory() {
    let dir = TempDir::new().unwrap();

    let result = load_skill_from_disk(dir.path(), "nonexistent");
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

    let result = load_skill_from_disk(dir.path(), "override-skill");
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

    let result = load_skill_from_disk(dir.path(), "frontmatter-skill");
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

    let result = load_skill_from_disk(dir.path(), "fallback-skill");
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

    let result = load_skill_from_disk(dir.path(), "name-test");
    let skill = result.expect("expected success");

    assert_eq!(skill.name, "Custom Name");
}

#[test]
fn test_load_skill_from_disk_name_defaults_to_skill_id() {
    let dir = TempDir::new().unwrap();
    let skill_dir = dir.path().join("default-name");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(skill_dir.join("SKILL.md"), "content").unwrap();

    let result = load_skill_from_disk(dir.path(), "default-name");
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

    let result = load_skill_from_disk(dir.path(), "bad-config");
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

    let result = load_skill_from_disk(dir.path(), "sec-test");
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

    let result = load_skill_from_disk(dir.path(), "empty-tags");
    let skill = result.expect("expected success");

    assert!(skill.tags.is_empty());
}
