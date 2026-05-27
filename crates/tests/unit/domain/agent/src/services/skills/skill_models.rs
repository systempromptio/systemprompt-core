use systemprompt_agent::services::skills::SkillMetadata;
use systemprompt_identifiers::SkillId;
use systemprompt_models::{DiskSkillConfig, IngestionReport, strip_frontmatter};

#[test]
fn test_strip_frontmatter_with_yaml_block() {
    let content = "---\ntitle: Test\n---\nActual content here";
    let result = strip_frontmatter(content);
    assert_eq!(result, "Actual content here");
}

#[test]
fn test_strip_frontmatter_no_frontmatter() {
    let content = "Just regular content";
    let result = strip_frontmatter(content);
    assert_eq!(result, "Just regular content");
}

#[test]
fn test_strip_frontmatter_empty_frontmatter() {
    let content = "---\n---\nBody text";
    let result = strip_frontmatter(content);
    assert_eq!(result, "Body text");
}

#[test]
fn test_strip_frontmatter_multiline_body() {
    let content = "---\nkey: value\n---\nLine 1\nLine 2\nLine 3";
    let result = strip_frontmatter(content);
    assert_eq!(result, "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_strip_frontmatter_trims_whitespace() {
    let content = "---\nkey: value\n---\n\n  Body with whitespace  \n\n";
    let result = strip_frontmatter(content);
    assert_eq!(result, "Body with whitespace");
}

#[test]
fn test_strip_frontmatter_single_separator() {
    let content = "---\nNo closing separator";
    let result = strip_frontmatter(content);
    assert_eq!(result, "---\nNo closing separator");
}

#[test]
fn test_strip_frontmatter_empty_string() {
    let result = strip_frontmatter("");
    assert_eq!(result, "");
}

#[test]
fn test_disk_skill_config_deserialize_full() {
    let yaml = r#"
id: my_skill
name: My Skill
description: A test skill
enabled: true
file: content.md
tags:
  - writing
  - blog
category: content
"#;
    let config: DiskSkillConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.id, "my_skill");
    assert_eq!(config.name, "My Skill");
    assert_eq!(config.description, "A test skill");
    assert!(config.enabled);
    assert_eq!(config.file, "content.md");
    assert_eq!(config.tags.len(), 2);
    assert_eq!(config.category, Some("content".to_string()));
}

#[test]
fn test_disk_skill_config_deserialize_minimal() {
    let yaml = r#"
id: minimal
name: Minimal
description: Bare minimum
"#;
    let config: DiskSkillConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.id, "minimal");
    assert!(config.enabled);
    assert!(config.file.is_empty());
    assert!(config.tags.is_empty());
    assert!(config.category.is_none());
}

#[test]
fn test_disk_skill_config_content_file_default() {
    let yaml = r#"
id: default_file
name: Default File
description: Uses default content file
"#;
    let config: DiskSkillConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.content_file(), "index.md");
}

#[test]
fn test_disk_skill_config_content_file_custom() {
    let yaml = r#"
id: custom_file
name: Custom File
description: Uses custom content file
file: custom.md
"#;
    let config: DiskSkillConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.content_file(), "custom.md");
}

#[test]
fn test_disk_skill_config_disabled() {
    let yaml = r#"
id: disabled
name: Disabled Skill
description: This skill is disabled
enabled: false
"#;
    let config: DiskSkillConfig = serde_yaml::from_str(yaml).unwrap();
    assert!(!config.enabled);
}

#[test]
fn test_ingestion_report_new() {
    let report = IngestionReport::new();
    assert_eq!(report.files_found, 0);
    assert_eq!(report.files_processed, 0);
    assert!(report.errors.is_empty());
}

#[test]
fn test_ingestion_report_has_errors_false() {
    let report = IngestionReport::new();
    assert!(!report.has_errors());
}

#[test]
fn test_ingestion_report_has_errors_true() {
    let mut report = IngestionReport::new();
    report.errors.push("something failed".to_string());
    assert!(report.has_errors());
}

#[test]
fn test_ingestion_report_successful_count() {
    let mut report = IngestionReport::new();
    report.files_processed = 5;
    report.errors.push("err1".to_string());
    report.errors.push("err2".to_string());
    assert_eq!(report.successful_count(), 3);
}

#[test]
fn test_ingestion_report_successful_count_no_errors() {
    let mut report = IngestionReport::new();
    report.files_processed = 10;
    assert_eq!(report.successful_count(), 10);
}

#[test]
fn test_ingestion_report_failed_count() {
    let mut report = IngestionReport::new();
    report.errors.push("fail1".to_string());
    report.errors.push("fail2".to_string());
    report.errors.push("fail3".to_string());
    assert_eq!(report.failed_count(), 3);
}

#[test]
fn test_ingestion_report_failed_count_zero() {
    let report = IngestionReport::new();
    assert_eq!(report.failed_count(), 0);
}

#[test]
fn test_ingestion_report_successful_count_saturating() {
    let mut report = IngestionReport::new();
    report.files_processed = 0;
    report.errors.push("err".to_string());
    assert_eq!(report.successful_count(), 0);
}

#[test]
fn test_skill_metadata_service_serialize() {
    let metadata = SkillMetadata {
        skill_id: SkillId::new("blog_writing"),
        name: "Blog Writing".to_string(),
    };
    let json = serde_json::to_string(&metadata).unwrap();
    assert!(json.contains("blog_writing"));
    assert!(json.contains("Blog Writing"));
}

#[test]
fn test_skill_metadata_service_deserialize() {
    let json = r#"{"skill_id": "code_review", "name": "Code Review"}"#;
    let metadata: SkillMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(metadata.skill_id.as_str(), "code_review");
    assert_eq!(metadata.name, "Code Review");
}

#[test]
fn test_skill_metadata_service_roundtrip() {
    let original = SkillMetadata {
        skill_id: SkillId::new("roundtrip_skill"),
        name: "Roundtrip".to_string(),
    };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: SkillMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.skill_id.as_str(), original.skill_id.as_str());
    assert_eq!(deserialized.name, original.name);
}

#[test]
fn test_skill_metadata_service_clone() {
    let metadata = SkillMetadata {
        skill_id: SkillId::new("clone_test"),
        name: "Clone Test".to_string(),
    };
    let cloned = metadata.clone();
    assert_eq!(cloned.skill_id.as_str(), metadata.skill_id.as_str());
    assert_eq!(cloned.name, metadata.name);
}

#[test]
fn test_skill_metadata_service_debug() {
    let metadata = SkillMetadata {
        skill_id: SkillId::new("debug_test"),
        name: "Debug".to_string(),
    };
    let debug = format!("{:?}", metadata);
    assert!(debug.contains("SkillMetadata"));
    assert!(debug.contains("debug_test"));
}
