//! Unit tests for Skill models
//!
//! Tests cover:
//! - SkillMetadata serialization/deserialization
//! - Skill structure validation
//! - Skill::from_json_row error cases

use systemprompt_agent::models::skill::SkillMetadata;

// ============================================================================
// SkillMetadata Tests
// ============================================================================

#[test]
fn test_skill_metadata_serialize() {
    let metadata = SkillMetadata {
        id: "skill-123".to_string(),
        name: "Search".to_string(),
        description: "Search capability".to_string(),
        enabled: true,
        file: "/skills/search.md".to_string(),
        assigned_agents: vec!["agent-1".to_string(), "agent-2".to_string()],
        tags: vec!["search".to_string(), "query".to_string()],
    };

    let json = serde_json::to_string(&metadata).unwrap();
    assert!(json.contains("skill-123"));
    assert!(json.contains("Search"));
    assert!(json.contains("search.md"));
    assert!(json.contains("agent-1"));
}

#[test]
fn test_skill_metadata_deserialize() {
    let json = r#"{
        "id": "skill-456",
        "name": "Chat",
        "description": "Chat capability",
        "enabled": false,
        "file": "/skills/chat.md",
        "assigned_agents": ["agent-3"],
        "tags": ["chat"]
    }"#;

    let metadata: SkillMetadata = serde_json::from_str(json).unwrap();
    assert_eq!(metadata.id, "skill-456");
    assert_eq!(metadata.name, "Chat");
    assert!(!metadata.enabled);
    assert_eq!(metadata.assigned_agents.len(), 1);
    assert_eq!(metadata.tags.len(), 1);
}

#[test]
fn test_skill_metadata_empty_arrays() {
    let json = r#"{
        "id": "skill-789",
        "name": "Empty",
        "description": "No agents or tags",
        "enabled": true,
        "file": "/skills/empty.md",
        "assigned_agents": [],
        "tags": []
    }"#;

    let metadata: SkillMetadata = serde_json::from_str(json).unwrap();
    assert!(metadata.assigned_agents.is_empty());
    assert!(metadata.tags.is_empty());
}

#[test]
fn test_skill_metadata_debug() {
    let metadata = SkillMetadata {
        id: "skill-debug".to_string(),
        name: "Debug".to_string(),
        description: "Debug skill".to_string(),
        enabled: true,
        file: "/debug.md".to_string(),
        assigned_agents: vec![],
        tags: vec![],
    };

    let debug = format!("{:?}", metadata);
    assert!(debug.contains("SkillMetadata"));
    assert!(debug.contains("skill-debug"));
}

#[test]
fn test_skill_metadata_clone() {
    let metadata = SkillMetadata {
        id: "skill-clone".to_string(),
        name: "Clone".to_string(),
        description: "Clone skill".to_string(),
        enabled: true,
        file: "/clone.md".to_string(),
        assigned_agents: vec!["agent".to_string()],
        tags: vec!["tag".to_string()],
    };

    let cloned = metadata.clone();
    assert_eq!(metadata.id, cloned.id);
    assert_eq!(metadata.name, cloned.name);
    assert_eq!(metadata.enabled, cloned.enabled);
}

#[test]
fn test_skill_metadata_multiple_agents() {
    let metadata = SkillMetadata {
        id: "skill-multi".to_string(),
        name: "Multi".to_string(),
        description: "Multi-agent skill".to_string(),
        enabled: true,
        file: "/multi.md".to_string(),
        assigned_agents: vec![
            "agent-a".to_string(),
            "agent-b".to_string(),
            "agent-c".to_string(),
        ],
        tags: vec!["a".to_string(), "b".to_string()],
    };

    assert_eq!(metadata.assigned_agents.len(), 3);
    assert!(metadata.assigned_agents.contains(&"agent-b".to_string()));
}

#[test]
fn test_skill_metadata_roundtrip() {
    let original = SkillMetadata {
        id: "skill-roundtrip".to_string(),
        name: "Roundtrip".to_string(),
        description: "Roundtrip test".to_string(),
        enabled: false,
        file: "/roundtrip.md".to_string(),
        assigned_agents: vec!["x".to_string()],
        tags: vec!["y".to_string()],
    };

    let json = serde_json::to_string(&original).unwrap();
    let parsed: SkillMetadata = serde_json::from_str(&json).unwrap();

    assert_eq!(original.id, parsed.id);
    assert_eq!(original.name, parsed.name);
    assert_eq!(original.description, parsed.description);
    assert_eq!(original.enabled, parsed.enabled);
    assert_eq!(original.file, parsed.file);
    assert_eq!(original.assigned_agents, parsed.assigned_agents);
    assert_eq!(original.tags, parsed.tags);
}

// ============================================================================
// Skill from_json_row Tests (error paths)
// ============================================================================

#[test]
fn test_skill_from_json_row_missing_skill_id() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert(
        "name".to_string(),
        serde_json::json!("Test"),
    );

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("skill_id"));
}

#[test]
fn test_skill_from_json_row_missing_file_path() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("file_path"));
}

#[test]
fn test_skill_from_json_row_missing_name() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("name"));
}

#[test]
fn test_skill_from_json_row_missing_description() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));
    row.insert("name".to_string(), serde_json::json!("Test"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("description"));
}

#[test]
fn test_skill_from_json_row_missing_instructions() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));
    row.insert("name".to_string(), serde_json::json!("Test"));
    row.insert("description".to_string(), serde_json::json!("Desc"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("instructions"));
}

#[test]
fn test_skill_from_json_row_missing_enabled() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));
    row.insert("name".to_string(), serde_json::json!("Test"));
    row.insert("description".to_string(), serde_json::json!("Desc"));
    row.insert("instructions".to_string(), serde_json::json!("Instr"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("enabled"));
}

#[test]
fn test_skill_from_json_row_missing_source_id() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));
    row.insert("name".to_string(), serde_json::json!("Test"));
    row.insert("description".to_string(), serde_json::json!("Desc"));
    row.insert("instructions".to_string(), serde_json::json!("Instr"));
    row.insert("enabled".to_string(), serde_json::json!(true));
    row.insert("tags".to_string(), serde_json::json!([]));

    let result = Skill::from_json_row(&row);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("source_id"));
}

#[test]
fn test_skill_from_json_row_default_tags() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-1"));
    row.insert("file_path".to_string(), serde_json::json!("/test.md"));
    row.insert("name".to_string(), serde_json::json!("Test"));
    row.insert("description".to_string(), serde_json::json!("Desc"));
    row.insert("instructions".to_string(), serde_json::json!("Instr"));
    row.insert("enabled".to_string(), serde_json::json!(true));
    // No tags field - should default to empty
    row.insert("source_id".to_string(), serde_json::json!("src-1"));
    row.insert("created_at".to_string(), serde_json::json!("2024-01-01T00:00:00Z"));
    row.insert("updated_at".to_string(), serde_json::json!("2024-01-01T00:00:00Z"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_ok());
    let skill = result.unwrap();
    assert!(skill.tags.is_empty());
}

#[test]
fn test_skill_from_json_row_complete() {
    use systemprompt_agent::models::skill::Skill;
    use systemprompt_database::JsonRow;

    let mut row = JsonRow::new();
    row.insert("skill_id".to_string(), serde_json::json!("sk-complete"));
    row.insert("file_path".to_string(), serde_json::json!("/complete.md"));
    row.insert("name".to_string(), serde_json::json!("Complete Skill"));
    row.insert("description".to_string(), serde_json::json!("A complete skill"));
    row.insert("instructions".to_string(), serde_json::json!("Do something"));
    row.insert("enabled".to_string(), serde_json::json!(true));
    row.insert("tags".to_string(), serde_json::json!(["tag1", "tag2"]));
    row.insert("category_id".to_string(), serde_json::json!("cat-1"));
    row.insert("source_id".to_string(), serde_json::json!("src-1"));
    row.insert("created_at".to_string(), serde_json::json!("2024-01-01T00:00:00Z"));
    row.insert("updated_at".to_string(), serde_json::json!("2024-01-02T00:00:00Z"));

    let result = Skill::from_json_row(&row);
    assert!(result.is_ok());
    let skill = result.unwrap();
    assert_eq!(skill.name, "Complete Skill");
    assert_eq!(skill.tags.len(), 2);
    assert!(skill.category_id.is_some());
}
