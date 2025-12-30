//! Unit tests for systemprompt-sync crate
//!
//! Tests cover:
//! - SyncError variants and error messages
//! - SyncConfig and SyncConfigBuilder
//! - SyncDirection enum and serialization
//! - SyncOperationResult construction and methods
//! - FileBundle, FileManifest, FileEntry models
//! - DatabaseExport models (AgentExport, SkillExport, ContextExport)
//! - LocalSync models (DiffStatus, ContentDiffItem, SkillDiffItem, etc.)
//! - Hash computation functions
//! - Export generation functions
//! - YAML escaping

use chrono::{TimeZone, Utc};
use std::fs;
use systemprompt_sync::{
    compute_content_hash, escape_yaml, export_content_to_file, export_skill_to_disk,
    generate_content_markdown, generate_skill_config, generate_skill_markdown, AgentExport,
    ContentDiffItem, ContentDiffResult, ContextExport, DatabaseExport, DiffStatus, DiskContent,
    DiskSkill, FileBundle, FileEntry, FileManifest, LocalSyncDirection, LocalSyncResult,
    SkillDiffItem, SkillExport, SkillsDiffResult, SyncConfig, SyncDirection, SyncError,
    SyncOperationResult,
};
use tempfile::TempDir;
use uuid::Uuid;

// ============================================================================
// SyncError Tests
// ============================================================================

#[test]
fn test_sync_error_database_url_missing() {
    let error = SyncError::DatabaseUrlMissing;
    assert_eq!(error.to_string(), "Database URL not configured");
}

#[test]
fn test_sync_error_api_error() {
    let error = SyncError::ApiError {
        status: 500,
        message: "Internal server error".to_string(),
    };
    assert_eq!(error.to_string(), "API error 500: Internal server error");
}

#[test]
fn test_sync_error_api_error_empty_message() {
    let error = SyncError::ApiError {
        status: 404,
        message: String::new(),
    };
    assert_eq!(error.to_string(), "API error 404: ");
}

#[test]
fn test_sync_error_unauthorized() {
    let error = SyncError::Unauthorized;
    assert_eq!(
        error.to_string(),
        "Unauthorized - run 'systemprompt cloud login'"
    );
}

#[test]
fn test_sync_error_tenant_no_app() {
    let error = SyncError::TenantNoApp;
    assert_eq!(error.to_string(), "Tenant has no associated app");
}

#[test]
fn test_sync_error_not_project_root() {
    let error = SyncError::NotProjectRoot;
    assert_eq!(
        error.to_string(),
        "Must run from project root (with infrastructure/ directory)"
    );
}

#[test]
fn test_sync_error_command_failed() {
    let error = SyncError::CommandFailed {
        command: "docker build".to_string(),
    };
    assert_eq!(error.to_string(), "Command failed: docker build");
}

#[test]
fn test_sync_error_docker_login_failed() {
    let error = SyncError::DockerLoginFailed;
    assert_eq!(error.to_string(), "Docker login failed");
}

#[test]
fn test_sync_error_git_sha_unavailable() {
    let error = SyncError::GitShaUnavailable;
    assert_eq!(error.to_string(), "Git SHA unavailable");
}

// ============================================================================
// SyncConfig and Builder Tests
// ============================================================================

#[test]
fn test_sync_config_builder_defaults() {
    let config = SyncConfig::builder(
        "tenant-123",
        "https://api.example.com",
        "secret-token",
        "/path/to/services",
    )
    .build();

    assert_eq!(config.tenant_id, "tenant-123");
    assert_eq!(config.api_url, "https://api.example.com");
    assert_eq!(config.api_token, "secret-token");
    assert_eq!(config.services_path, "/path/to/services");
    assert_eq!(config.direction, SyncDirection::Push);
    assert!(!config.dry_run);
    assert!(!config.verbose);
    assert!(config.database_url.is_none());
}

#[test]
fn test_sync_config_builder_with_all_options() {
    let config = SyncConfig::builder("tenant", "https://api.com", "token", "/services")
        .with_direction(SyncDirection::Pull)
        .with_dry_run(true)
        .with_verbose(true)
        .with_database_url("postgres://localhost/db")
        .build();

    assert_eq!(config.direction, SyncDirection::Pull);
    assert!(config.dry_run);
    assert!(config.verbose);
    assert_eq!(
        config.database_url,
        Some("postgres://localhost/db".to_string())
    );
}

#[test]
fn test_sync_config_builder_string_conversions() {
    let config = SyncConfig::builder(
        String::from("tenant"),
        String::from("https://api.com"),
        String::from("token"),
        String::from("/services"),
    )
    .build();

    assert_eq!(config.tenant_id, "tenant");
}

#[test]
fn test_sync_config_builder_chain_order_irrelevant() {
    let config1 = SyncConfig::builder("t", "u", "p", "s")
        .with_dry_run(true)
        .with_verbose(true)
        .build();

    let config2 = SyncConfig::builder("t", "u", "p", "s")
        .with_verbose(true)
        .with_dry_run(true)
        .build();

    assert_eq!(config1.dry_run, config2.dry_run);
    assert_eq!(config1.verbose, config2.verbose);
}

// ============================================================================
// SyncDirection Tests
// ============================================================================

#[test]
fn test_sync_direction_equality() {
    assert_eq!(SyncDirection::Push, SyncDirection::Push);
    assert_eq!(SyncDirection::Pull, SyncDirection::Pull);
    assert_ne!(SyncDirection::Push, SyncDirection::Pull);
}

#[test]
fn test_sync_direction_clone() {
    let push = SyncDirection::Push;
    let cloned = push;
    assert_eq!(push, cloned);
}

#[test]
fn test_sync_direction_serialization() {
    let push = SyncDirection::Push;
    let pull = SyncDirection::Pull;

    let push_json = serde_json::to_string(&push).unwrap();
    let pull_json = serde_json::to_string(&pull).unwrap();

    assert_eq!(push_json, "\"Push\"");
    assert_eq!(pull_json, "\"Pull\"");
}

#[test]
fn test_sync_direction_deserialization() {
    let push: SyncDirection = serde_json::from_str("\"Push\"").unwrap();
    let pull: SyncDirection = serde_json::from_str("\"Pull\"").unwrap();

    assert_eq!(push, SyncDirection::Push);
    assert_eq!(pull, SyncDirection::Pull);
}

#[test]
fn test_sync_direction_invalid_deserialization() {
    let result: Result<SyncDirection, _> = serde_json::from_str("\"Invalid\"");
    assert!(result.is_err());
}

// ============================================================================
// SyncOperationResult Tests
// ============================================================================

#[test]
fn test_sync_operation_result_success() {
    let result = SyncOperationResult::success("files_push", 10);

    assert_eq!(result.operation, "files_push");
    assert!(result.success);
    assert_eq!(result.items_synced, 10);
    assert_eq!(result.items_skipped, 0);
    assert!(result.errors.is_empty());
    assert!(result.details.is_none());
}

#[test]
fn test_sync_operation_result_with_details() {
    let details = serde_json::json!({
        "files": ["a.txt", "b.txt"],
        "total_size": 1024
    });
    let result = SyncOperationResult::success("files_push", 2).with_details(details.clone());

    assert_eq!(result.details, Some(details));
    assert!(result.success);
}

#[test]
fn test_sync_operation_result_dry_run() {
    let details = serde_json::json!({
        "preview": true,
        "files": ["test.md"]
    });
    let result = SyncOperationResult::dry_run("database_push", 5, details.clone());

    assert!(result.success);
    assert_eq!(result.items_synced, 0);
    assert_eq!(result.items_skipped, 5);
    assert_eq!(result.details, Some(details));
}

#[test]
fn test_sync_operation_result_chained_with_details() {
    let result = SyncOperationResult::success("test", 1)
        .with_details(serde_json::json!({"key": "value1"}))
        .with_details(serde_json::json!({"key": "value2"}));

    // Second with_details should override
    assert_eq!(result.details, Some(serde_json::json!({"key": "value2"})));
}

#[test]
fn test_sync_operation_result_serialization() {
    let result = SyncOperationResult::success("test_op", 5);
    let json = serde_json::to_string(&result).unwrap();

    assert!(json.contains("\"operation\":\"test_op\""));
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"items_synced\":5"));
}

// ============================================================================
// FileManifest and FileEntry Tests
// ============================================================================

#[test]
fn test_file_entry_creation() {
    let entry = FileEntry {
        path: "agents/default/config.yaml".to_string(),
        checksum: "abc123def456".to_string(),
        size: 1024,
    };

    assert_eq!(entry.path, "agents/default/config.yaml");
    assert_eq!(entry.checksum, "abc123def456");
    assert_eq!(entry.size, 1024);
}

#[test]
fn test_file_entry_serialization() {
    let entry = FileEntry {
        path: "test.md".to_string(),
        checksum: "hash".to_string(),
        size: 100,
    };

    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"path\":\"test.md\""));
    assert!(json.contains("\"checksum\":\"hash\""));
    assert!(json.contains("\"size\":100"));
}

#[test]
fn test_file_manifest_creation() {
    let now = Utc::now();
    let manifest = FileManifest {
        files: vec![
            FileEntry {
                path: "file1.txt".to_string(),
                checksum: "hash1".to_string(),
                size: 100,
            },
            FileEntry {
                path: "file2.txt".to_string(),
                checksum: "hash2".to_string(),
                size: 200,
            },
        ],
        timestamp: now,
        checksum: "manifest_hash".to_string(),
    };

    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.checksum, "manifest_hash");
}

#[test]
fn test_file_manifest_empty() {
    let manifest = FileManifest {
        files: vec![],
        timestamp: Utc::now(),
        checksum: "empty_hash".to_string(),
    };

    assert!(manifest.files.is_empty());
}

#[test]
fn test_file_bundle_creation() {
    let bundle = FileBundle {
        manifest: FileManifest {
            files: vec![],
            timestamp: Utc::now(),
            checksum: "test".to_string(),
        },
        data: vec![1, 2, 3, 4],
    };

    assert_eq!(bundle.data.len(), 4);
}

// ============================================================================
// DatabaseExport Models Tests
// ============================================================================

#[test]
fn test_agent_export_creation() {
    let now = Utc::now();
    let agent = AgentExport {
        id: Uuid::new_v4(),
        name: "Test Agent".to_string(),
        system_prompt: Some("You are a helpful assistant".to_string()),
        created_at: now,
        updated_at: now,
    };

    assert_eq!(agent.name, "Test Agent");
    assert!(agent.system_prompt.is_some());
}

#[test]
fn test_agent_export_no_system_prompt() {
    let now = Utc::now();
    let agent = AgentExport {
        id: Uuid::new_v4(),
        name: "Minimal Agent".to_string(),
        system_prompt: None,
        created_at: now,
        updated_at: now,
    };

    assert!(agent.system_prompt.is_none());
}

#[test]
fn test_skill_export_creation() {
    let now = Utc::now();
    let skill = SkillExport {
        id: Uuid::new_v4(),
        agent_id: Uuid::new_v4(),
        name: "Test Skill".to_string(),
        description: Some("A skill for testing".to_string()),
        created_at: now,
        updated_at: now,
    };

    assert_eq!(skill.name, "Test Skill");
    assert!(skill.description.is_some());
}

#[test]
fn test_context_export_creation() {
    let now = Utc::now();
    let context = ContextExport {
        id: Uuid::new_v4(),
        name: "Test Context".to_string(),
        description: Some("Context description".to_string()),
        created_at: now,
        updated_at: now,
    };

    assert_eq!(context.name, "Test Context");
}

#[test]
fn test_database_export_full() {
    let now = Utc::now();
    let agent_id = Uuid::new_v4();

    let export = DatabaseExport {
        agents: vec![AgentExport {
            id: agent_id,
            name: "Agent".to_string(),
            system_prompt: Some("prompt".to_string()),
            created_at: now,
            updated_at: now,
        }],
        skills: vec![SkillExport {
            id: Uuid::new_v4(),
            agent_id,
            name: "Skill".to_string(),
            description: None,
            created_at: now,
            updated_at: now,
        }],
        contexts: vec![ContextExport {
            id: Uuid::new_v4(),
            name: "Context".to_string(),
            description: None,
            created_at: now,
            updated_at: now,
        }],
        timestamp: now,
    };

    assert_eq!(export.agents.len(), 1);
    assert_eq!(export.skills.len(), 1);
    assert_eq!(export.contexts.len(), 1);
}

#[test]
fn test_database_export_empty() {
    let export = DatabaseExport {
        agents: vec![],
        skills: vec![],
        contexts: vec![],
        timestamp: Utc::now(),
    };

    assert!(export.agents.is_empty());
    assert!(export.skills.is_empty());
    assert!(export.contexts.is_empty());
}

#[test]
fn test_database_export_serialization() {
    let now = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
    let export = DatabaseExport {
        agents: vec![],
        skills: vec![],
        contexts: vec![],
        timestamp: now,
    };

    let json = serde_json::to_string(&export).unwrap();
    assert!(json.contains("\"agents\":[]"));
    assert!(json.contains("\"skills\":[]"));
    assert!(json.contains("\"contexts\":[]"));
}

// ============================================================================
// LocalSync Models Tests
// ============================================================================

#[test]
fn test_local_sync_direction_variants() {
    assert_eq!(LocalSyncDirection::ToDisk, LocalSyncDirection::ToDisk);
    assert_eq!(
        LocalSyncDirection::ToDatabase,
        LocalSyncDirection::ToDatabase
    );
    assert_ne!(LocalSyncDirection::ToDisk, LocalSyncDirection::ToDatabase);
}

#[test]
fn test_diff_status_variants() {
    assert_eq!(DiffStatus::Added, DiffStatus::Added);
    assert_eq!(DiffStatus::Removed, DiffStatus::Removed);
    assert_eq!(DiffStatus::Modified, DiffStatus::Modified);
    assert_ne!(DiffStatus::Added, DiffStatus::Removed);
}

#[test]
fn test_diff_status_serialization() {
    let added = serde_json::to_string(&DiffStatus::Added).unwrap();
    let removed = serde_json::to_string(&DiffStatus::Removed).unwrap();
    let modified = serde_json::to_string(&DiffStatus::Modified).unwrap();

    assert_eq!(added, "\"Added\"");
    assert_eq!(removed, "\"Removed\"");
    assert_eq!(modified, "\"Modified\"");
}

#[test]
fn test_content_diff_item_added() {
    let item = ContentDiffItem {
        slug: "new-article".to_string(),
        source_id: "blog".to_string(),
        status: DiffStatus::Added,
        disk_hash: Some("abc123".to_string()),
        db_hash: None,
        disk_updated_at: None,
        db_updated_at: None,
        title: Some("New Article".to_string()),
    };

    assert_eq!(item.status, DiffStatus::Added);
    assert!(item.disk_hash.is_some());
    assert!(item.db_hash.is_none());
}

#[test]
fn test_content_diff_item_removed() {
    let now = Utc::now();
    let item = ContentDiffItem {
        slug: "old-article".to_string(),
        source_id: "blog".to_string(),
        status: DiffStatus::Removed,
        disk_hash: None,
        db_hash: Some("def456".to_string()),
        disk_updated_at: None,
        db_updated_at: Some(now),
        title: Some("Old Article".to_string()),
    };

    assert_eq!(item.status, DiffStatus::Removed);
    assert!(item.disk_hash.is_none());
    assert!(item.db_hash.is_some());
}

#[test]
fn test_content_diff_item_modified() {
    let item = ContentDiffItem {
        slug: "updated-article".to_string(),
        source_id: "blog".to_string(),
        status: DiffStatus::Modified,
        disk_hash: Some("new_hash".to_string()),
        db_hash: Some("old_hash".to_string()),
        disk_updated_at: None,
        db_updated_at: None,
        title: Some("Updated Article".to_string()),
    };

    assert_eq!(item.status, DiffStatus::Modified);
    assert_ne!(item.disk_hash, item.db_hash);
}

#[test]
fn test_skill_diff_item_creation() {
    let item = SkillDiffItem {
        skill_id: "new_skill".to_string(),
        file_path: "/skills/new-skill/index.md".to_string(),
        status: DiffStatus::Added,
        disk_hash: Some("hash123".to_string()),
        db_hash: None,
        name: Some("New Skill".to_string()),
    };

    assert_eq!(item.skill_id, "new_skill");
    assert_eq!(item.status, DiffStatus::Added);
}

#[test]
fn test_content_diff_result_no_changes() {
    let result = ContentDiffResult {
        source_id: "test-source".to_string(),
        added: vec![],
        removed: vec![],
        modified: vec![],
        unchanged: 5,
    };

    assert!(!result.has_changes());
    assert_eq!(result.unchanged, 5);
}

#[test]
fn test_content_diff_result_with_additions() {
    let result = ContentDiffResult {
        source_id: "test-source".to_string(),
        added: vec![ContentDiffItem {
            slug: "new".to_string(),
            source_id: "test".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash".to_string()),
            db_hash: None,
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("New".to_string()),
        }],
        removed: vec![],
        modified: vec![],
        unchanged: 0,
    };

    assert!(result.has_changes());
    assert_eq!(result.added.len(), 1);
}

#[test]
fn test_content_diff_result_with_removals() {
    let result = ContentDiffResult {
        source_id: "test-source".to_string(),
        added: vec![],
        removed: vec![ContentDiffItem {
            slug: "old".to_string(),
            source_id: "test".to_string(),
            status: DiffStatus::Removed,
            disk_hash: None,
            db_hash: Some("hash".to_string()),
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("Old".to_string()),
        }],
        modified: vec![],
        unchanged: 0,
    };

    assert!(result.has_changes());
    assert_eq!(result.removed.len(), 1);
}

#[test]
fn test_content_diff_result_with_modifications() {
    let result = ContentDiffResult {
        source_id: "test-source".to_string(),
        added: vec![],
        removed: vec![],
        modified: vec![ContentDiffItem {
            slug: "changed".to_string(),
            source_id: "test".to_string(),
            status: DiffStatus::Modified,
            disk_hash: Some("new".to_string()),
            db_hash: Some("old".to_string()),
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("Changed".to_string()),
        }],
        unchanged: 3,
    };

    assert!(result.has_changes());
    assert_eq!(result.modified.len(), 1);
    assert_eq!(result.unchanged, 3);
}

#[test]
fn test_skills_diff_result_no_changes() {
    let result = SkillsDiffResult::default();

    assert!(!result.has_changes());
    assert_eq!(result.unchanged, 0);
}

#[test]
fn test_skills_diff_result_with_changes() {
    let result = SkillsDiffResult {
        added: vec![SkillDiffItem {
            skill_id: "skill1".to_string(),
            file_path: "/skills/skill1/index.md".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash".to_string()),
            db_hash: None,
            name: Some("Skill 1".to_string()),
        }],
        removed: vec![],
        modified: vec![],
        unchanged: 2,
    };

    assert!(result.has_changes());
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.unchanged, 2);
}

#[test]
fn test_local_sync_result_default() {
    let result = LocalSyncResult::default();

    assert_eq!(result.items_synced, 0);
    assert_eq!(result.items_skipped, 0);
    assert_eq!(result.items_deleted, 0);
    assert!(result.errors.is_empty());
    assert!(result.direction.is_empty());
}

#[test]
fn test_local_sync_result_with_values() {
    let result = LocalSyncResult {
        items_synced: 10,
        items_skipped: 2,
        items_deleted: 1,
        errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        direction: "to_disk".to_string(),
    };

    assert_eq!(result.items_synced, 10);
    assert_eq!(result.items_skipped, 2);
    assert_eq!(result.items_deleted, 1);
    assert_eq!(result.errors.len(), 2);
    assert_eq!(result.direction, "to_disk");
}

#[test]
fn test_disk_content_model() {
    let content = DiskContent {
        slug: "test-article".to_string(),
        title: "Test Article".to_string(),
        body: "Article body content".to_string(),
    };

    assert_eq!(content.slug, "test-article");
    assert_eq!(content.title, "Test Article");
    assert_eq!(content.body, "Article body content");
}

#[test]
fn test_disk_skill_model() {
    let skill = DiskSkill {
        skill_id: "test_skill".to_string(),
        name: "Test Skill".to_string(),
        description: "A test skill".to_string(),
        instructions: "Do something useful".to_string(),
        file_path: "/skills/test-skill/index.md".to_string(),
    };

    assert_eq!(skill.skill_id, "test_skill");
    assert_eq!(skill.name, "Test Skill");
    assert_eq!(skill.description, "A test skill");
    assert_eq!(skill.instructions, "Do something useful");
    assert_eq!(skill.file_path, "/skills/test-skill/index.md");
}

// ============================================================================
// Hash Computation Tests
// ============================================================================

#[test]
fn test_compute_content_hash_basic() {
    let body = "This is the content body";
    let title = "Test Title";

    let hash = compute_content_hash(body, title);

    // Hash should be a 64-character hex string (SHA256)
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_compute_content_hash_consistency() {
    let body = "Same content";
    let title = "Same title";

    let hash1 = compute_content_hash(body, title);
    let hash2 = compute_content_hash(body, title);

    assert_eq!(hash1, hash2);
}

#[test]
fn test_compute_content_hash_different_content() {
    let body1 = "Content A";
    let body2 = "Content B";
    let title = "Same title";

    let hash1 = compute_content_hash(body1, title);
    let hash2 = compute_content_hash(body2, title);

    assert_ne!(hash1, hash2);
}

#[test]
fn test_compute_content_hash_different_title() {
    let body = "Same content";
    let title1 = "Title A";
    let title2 = "Title B";

    let hash1 = compute_content_hash(body, title1);
    let hash2 = compute_content_hash(body, title2);

    assert_ne!(hash1, hash2);
}

#[test]
fn test_compute_content_hash_empty() {
    let hash = compute_content_hash("", "");

    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_compute_content_hash_unicode() {
    let body = "こんにちは世界";
    let title = "日本語タイトル";

    let hash = compute_content_hash(body, title);

    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_compute_content_hash_whitespace_matters() {
    let hash1 = compute_content_hash("test", "title");
    let hash2 = compute_content_hash("test ", "title");
    let hash3 = compute_content_hash(" test", "title");

    assert_ne!(hash1, hash2);
    assert_ne!(hash1, hash3);
    assert_ne!(hash2, hash3);
}

#[test]
fn test_compute_content_hash_order_matters() {
    let hash1 = compute_content_hash("body", "title");
    let hash2 = compute_content_hash("title", "body");

    assert_ne!(hash1, hash2);
}

// ============================================================================
// YAML Escape Tests
// ============================================================================

#[test]
fn test_escape_yaml_plain_string() {
    let input = "Simple text";
    let output = escape_yaml(input);

    assert_eq!(output, "Simple text");
}

#[test]
fn test_escape_yaml_backslash() {
    let input = r"Path\to\file";
    let output = escape_yaml(input);

    assert_eq!(output, r"Path\\to\\file");
}

#[test]
fn test_escape_yaml_quotes() {
    let input = r#"Say "hello""#;
    let output = escape_yaml(input);

    assert_eq!(output, r#"Say \"hello\""#);
}

#[test]
fn test_escape_yaml_newlines() {
    let input = "Line1\nLine2";
    let output = escape_yaml(input);

    assert_eq!(output, r"Line1\nLine2");
}

#[test]
fn test_escape_yaml_combined() {
    let input = "Path\\to\\file \"with\nnewline\"";
    let output = escape_yaml(input);

    assert_eq!(output, r#"Path\\to\\file \"with\nnewline\""#);
}

#[test]
fn test_escape_yaml_empty() {
    let input = "";
    let output = escape_yaml(input);

    assert_eq!(output, "");
}

#[test]
fn test_escape_yaml_multiple_escapes() {
    // Test that escape_yaml handles all character types together
    // Input: backslash, quote, newline
    let input = "a\\b\"c\nd";
    let output = escape_yaml(input);

    // a stays a
    // \ becomes \\
    // b stays b
    // " becomes \"
    // c stays c
    // \n becomes \n (literal backslash-n, not newline)
    // d stays d
    assert_eq!(output, r#"a\\b\"c\nd"#);
}

// ============================================================================
// Export Generation Tests (using tempfile)
// ============================================================================

#[test]
fn test_generate_skill_markdown_structure() {
    use systemprompt_core_agent::models::Skill;
    use systemprompt_identifiers::{CategoryId, SkillId, SourceId};

    let skill = Skill {
        skill_id: SkillId::new("test_skill"),
        file_path: "/skills/test-skill/index.md".to_string(),
        name: "Test Skill".to_string(),
        description: "A test skill description".to_string(),
        instructions: "Follow these instructions carefully.".to_string(),
        enabled: true,
        tags: vec!["tag1".to_string(), "tag2".to_string()],
        category_id: Some(CategoryId::new("skills")),
        source_id: SourceId::new("skills"),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let markdown = generate_skill_markdown(&skill);

    assert!(markdown.starts_with("---\n"));
    assert!(markdown.contains("title: \"Test Skill\""));
    assert!(markdown.contains("slug: \"test-skill\"")); // underscore to dash
    assert!(markdown.contains("description: \"A test skill description\""));
    assert!(markdown.contains("type: \"skill\""));
    assert!(markdown.contains("keywords: \"tag1, tag2\""));
    assert!(markdown.contains("Follow these instructions carefully."));
}

#[test]
fn test_generate_skill_config_structure() {
    use systemprompt_core_agent::models::Skill;
    use systemprompt_identifiers::{CategoryId, SkillId, SourceId};

    let skill = Skill {
        skill_id: SkillId::new("config_test"),
        file_path: "/skills/config-test/index.md".to_string(),
        name: "Config Test".to_string(),
        description: "Testing config generation".to_string(),
        instructions: "Instructions here".to_string(),
        enabled: true,
        tags: vec!["tag1".to_string()],
        category_id: Some(CategoryId::new("skills")),
        source_id: SourceId::new("skills"),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let config = generate_skill_config(&skill);

    assert!(config.contains("id: config_test"));
    assert!(config.contains("name: \"Config Test\""));
    assert!(config.contains("enabled: true"));
    assert!(config.contains("version: \"1.0.0\""));
    assert!(config.contains("file: \"index.md\""));
}

#[test]
fn test_generate_skill_config_empty_tags() {
    use systemprompt_core_agent::models::Skill;
    use systemprompt_identifiers::{SkillId, SourceId};

    let skill = Skill {
        skill_id: SkillId::new("no_tags"),
        file_path: "/skills/no-tags/index.md".to_string(),
        name: "No Tags".to_string(),
        description: "No tags skill".to_string(),
        instructions: "Instructions".to_string(),
        enabled: false,
        tags: vec![],
        category_id: None,
        source_id: SourceId::new("skills"),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let config = generate_skill_config(&skill);

    assert!(config.contains("tags:\n[]"));
    assert!(config.contains("enabled: false"));
}

#[test]
fn test_export_skill_to_disk_creates_files() {
    use systemprompt_core_agent::models::Skill;
    use systemprompt_identifiers::{CategoryId, SkillId, SourceId};

    let temp_dir = TempDir::new().unwrap();
    let skill = Skill {
        skill_id: SkillId::new("export_test"),
        file_path: "/skills/export-test/index.md".to_string(),
        name: "Export Test".to_string(),
        description: "Testing export".to_string(),
        instructions: "Test instructions".to_string(),
        enabled: true,
        tags: vec![],
        category_id: Some(CategoryId::new("skills")),
        source_id: SourceId::new("skills"),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let result = export_skill_to_disk(&skill, temp_dir.path());
    assert!(result.is_ok());

    let skill_dir = temp_dir.path().join("export-test");
    assert!(skill_dir.exists());
    assert!(skill_dir.join("index.md").exists());
    assert!(skill_dir.join("config.yaml").exists());

    let index_content = fs::read_to_string(skill_dir.join("index.md")).unwrap();
    assert!(index_content.contains("title: \"Export Test\""));

    let config_content = fs::read_to_string(skill_dir.join("config.yaml")).unwrap();
    assert!(config_content.contains("id: export_test"));
}

#[test]
fn test_export_skill_underscore_to_dash() {
    use systemprompt_core_agent::models::Skill;
    use systemprompt_identifiers::{SkillId, SourceId};

    let temp_dir = TempDir::new().unwrap();
    let skill = Skill {
        skill_id: SkillId::new("my_complex_skill_name"),
        file_path: "/skills/my-complex-skill-name/index.md".to_string(),
        name: "Complex Skill".to_string(),
        description: "Complex".to_string(),
        instructions: "Instructions".to_string(),
        enabled: true,
        tags: vec![],
        category_id: None,
        source_id: SourceId::new("skills"),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let result = export_skill_to_disk(&skill, temp_dir.path());
    assert!(result.is_ok());

    let skill_dir = temp_dir.path().join("my-complex-skill-name");
    assert!(skill_dir.exists());
}

#[test]
fn test_generate_content_markdown_structure() {
    use systemprompt_core_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};

    let content = Content {
        id: ContentId::new("test-id"),
        slug: "test-article".to_string(),
        title: "Test Article".to_string(),
        description: "Article description".to_string(),
        body: "Article body content goes here.".to_string(),
        author: "Test Author".to_string(),
        published_at: Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).unwrap(),
        keywords: "test, article".to_string(),
        kind: "article".to_string(),
        image: Some("cover.jpg".to_string()),
        category_id: None,
        source_id: SourceId::new("blog"),
        version_hash: "hash123".to_string(),
        links: serde_json::json!([]),
        updated_at: Some(Utc.with_ymd_and_hms(2024, 7, 20, 0, 0, 0).unwrap()),
    };

    let markdown = generate_content_markdown(&content);

    assert!(markdown.starts_with("---\n"));
    assert!(markdown.contains("title: \"Test Article\""));
    assert!(markdown.contains("slug: \"test-article\""));
    assert!(markdown.contains("author: \"Test Author\""));
    assert!(markdown.contains("published_at: \"2024-06-15\""));
    assert!(markdown.contains("updated_at: \"2024-07-20\""));
    assert!(markdown.contains("image: \"cover.jpg\""));
    assert!(markdown.contains("Article body content goes here."));
}

#[test]
fn test_generate_content_markdown_no_image() {
    use systemprompt_core_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};

    let content = Content {
        id: ContentId::new("no-image"),
        slug: "no-image".to_string(),
        title: "No Image".to_string(),
        description: "No image".to_string(),
        body: "Body".to_string(),
        author: "Author".to_string(),
        published_at: Utc::now(),
        keywords: "".to_string(),
        kind: "article".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("blog"),
        version_hash: "hash".to_string(),
        links: serde_json::json!([]),
        updated_at: None,
    };

    let markdown = generate_content_markdown(&content);

    assert!(markdown.contains("image: \"\""));
    assert!(markdown.contains("updated_at: \"\""));
}

#[test]
fn test_export_content_to_file_docs() {
    use systemprompt_core_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};

    let temp_dir = TempDir::new().unwrap();
    let content = Content {
        id: ContentId::new("doc-1"),
        slug: "getting-started".to_string(),
        title: "Getting Started".to_string(),
        description: "How to get started".to_string(),
        body: "Documentation content".to_string(),
        author: "Docs Team".to_string(),
        published_at: Utc::now(),
        keywords: "docs".to_string(),
        kind: "docs".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("docs"),
        version_hash: "hash".to_string(),
        links: serde_json::json!([]),
        updated_at: None,
    };

    let result = export_content_to_file(&content, temp_dir.path(), "docs");
    assert!(result.is_ok());

    let file_path = temp_dir.path().join("getting-started.md");
    assert!(file_path.exists());

    let file_content = fs::read_to_string(&file_path).unwrap();
    assert!(file_content.contains("title: \"Getting Started\""));
}

#[test]
fn test_export_content_to_file_blog_creates_directory() {
    use systemprompt_core_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};

    let temp_dir = TempDir::new().unwrap();
    let content = Content {
        id: ContentId::new("blog-1"),
        slug: "my-blog-post".to_string(),
        title: "My Blog Post".to_string(),
        description: "A blog post".to_string(),
        body: "Blog content".to_string(),
        author: "Blogger".to_string(),
        published_at: Utc::now(),
        keywords: "blog".to_string(),
        kind: "blog".to_string(),
        image: None,
        category_id: None,
        source_id: SourceId::new("blog"),
        version_hash: "hash".to_string(),
        links: serde_json::json!([]),
        updated_at: None,
    };

    let result = export_content_to_file(&content, temp_dir.path(), "blog");
    assert!(result.is_ok());

    let file_path = temp_dir.path().join("my-blog-post").join("index.md");
    assert!(file_path.exists());
}

// ============================================================================
// Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_empty_tenant_id() {
    let config = SyncConfig::builder("", "https://api.com", "token", "/services").build();

    assert_eq!(config.tenant_id, "");
}

#[test]
fn test_very_long_strings() {
    let long_string = "x".repeat(10000);
    let config = SyncConfig::builder(&long_string, "https://api.com", "token", "/services").build();

    assert_eq!(config.tenant_id.len(), 10000);
}

#[test]
fn test_special_characters_in_config() {
    let config = SyncConfig::builder(
        "tenant-123_special!@#",
        "https://api.example.com/v1",
        "token+with/special=chars",
        "/path/with spaces/and-dashes",
    )
    .build();

    assert_eq!(config.tenant_id, "tenant-123_special!@#");
    assert!(config.services_path.contains(" "));
}

#[test]
fn test_unicode_in_content_diff_item() {
    let item = ContentDiffItem {
        slug: "日本語スラッグ".to_string(),
        source_id: "ソース".to_string(),
        status: DiffStatus::Added,
        disk_hash: Some("ハッシュ".to_string()),
        db_hash: None,
        disk_updated_at: None,
        db_updated_at: None,
        title: Some("日本語タイトル".to_string()),
    };

    assert_eq!(item.slug, "日本語スラッグ");
    assert_eq!(item.title, Some("日本語タイトル".to_string()));
}

#[test]
fn test_file_entry_zero_size() {
    let entry = FileEntry {
        path: "empty.txt".to_string(),
        checksum: "empty_hash".to_string(),
        size: 0,
    };

    assert_eq!(entry.size, 0);
}

#[test]
fn test_file_entry_large_size() {
    let entry = FileEntry {
        path: "large.bin".to_string(),
        checksum: "large_hash".to_string(),
        size: u64::MAX,
    };

    assert_eq!(entry.size, u64::MAX);
}

#[test]
fn test_sync_operation_result_zero_items() {
    let result = SyncOperationResult::success("empty_sync", 0);

    assert!(result.success);
    assert_eq!(result.items_synced, 0);
}

#[test]
fn test_sync_operation_result_large_item_count() {
    let result = SyncOperationResult::success("large_sync", usize::MAX);

    assert_eq!(result.items_synced, usize::MAX);
}

#[test]
fn test_database_export_multiple_items() {
    let now = Utc::now();

    let agents: Vec<AgentExport> = (0..100)
        .map(|i| AgentExport {
            id: Uuid::new_v4(),
            name: format!("Agent {}", i),
            system_prompt: Some(format!("Prompt {}", i)),
            created_at: now,
            updated_at: now,
        })
        .collect();

    let export = DatabaseExport {
        agents,
        skills: vec![],
        contexts: vec![],
        timestamp: now,
    };

    assert_eq!(export.agents.len(), 100);
}

#[test]
fn test_content_diff_result_all_types() {
    let now = Utc::now();
    let result = ContentDiffResult {
        source_id: "mixed".to_string(),
        added: vec![ContentDiffItem {
            slug: "new".to_string(),
            source_id: "mixed".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("h1".to_string()),
            db_hash: None,
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("New".to_string()),
        }],
        removed: vec![ContentDiffItem {
            slug: "old".to_string(),
            source_id: "mixed".to_string(),
            status: DiffStatus::Removed,
            disk_hash: None,
            db_hash: Some("h2".to_string()),
            disk_updated_at: None,
            db_updated_at: Some(now),
            title: Some("Old".to_string()),
        }],
        modified: vec![ContentDiffItem {
            slug: "changed".to_string(),
            source_id: "mixed".to_string(),
            status: DiffStatus::Modified,
            disk_hash: Some("h3".to_string()),
            db_hash: Some("h4".to_string()),
            disk_updated_at: None,
            db_updated_at: Some(now),
            title: Some("Changed".to_string()),
        }],
        unchanged: 10,
    };

    assert!(result.has_changes());
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 1);
    assert_eq!(result.modified.len(), 1);
    assert_eq!(result.unchanged, 10);
}

// ============================================================================
// Serialization Round-Trip Tests
// ============================================================================

#[test]
fn test_sync_direction_roundtrip() {
    let original = SyncDirection::Push;
    let json = serde_json::to_string(&original).unwrap();
    let restored: SyncDirection = serde_json::from_str(&json).unwrap();

    assert_eq!(original, restored);
}

#[test]
fn test_diff_status_serialize_all_variants() {
    // DiffStatus only implements Serialize (not Deserialize), so we only test
    // serialization
    let added_json = serde_json::to_string(&DiffStatus::Added).unwrap();
    let removed_json = serde_json::to_string(&DiffStatus::Removed).unwrap();
    let modified_json = serde_json::to_string(&DiffStatus::Modified).unwrap();

    assert_eq!(added_json, "\"Added\"");
    assert_eq!(removed_json, "\"Removed\"");
    assert_eq!(modified_json, "\"Modified\"");
}

#[test]
fn test_file_manifest_roundtrip() {
    let now = Utc::now();
    let original = FileManifest {
        files: vec![FileEntry {
            path: "test.txt".to_string(),
            checksum: "abc123".to_string(),
            size: 1024,
        }],
        timestamp: now,
        checksum: "manifest123".to_string(),
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: FileManifest = serde_json::from_str(&json).unwrap();

    assert_eq!(original.files.len(), restored.files.len());
    assert_eq!(original.checksum, restored.checksum);
}

#[test]
fn test_database_export_roundtrip() {
    let now = Utc::now();
    let original = DatabaseExport {
        agents: vec![AgentExport {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            system_prompt: Some("Prompt".to_string()),
            created_at: now,
            updated_at: now,
        }],
        skills: vec![],
        contexts: vec![],
        timestamp: now,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: DatabaseExport = serde_json::from_str(&json).unwrap();

    assert_eq!(original.agents.len(), restored.agents.len());
    assert_eq!(original.agents[0].name, restored.agents[0].name);
}
