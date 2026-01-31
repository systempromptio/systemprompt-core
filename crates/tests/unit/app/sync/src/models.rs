//! Tests for sync models

use systemprompt_sync::{
    compute_content_hash, ContentDiffItem, ContentDiffResult, DiffStatus, LocalSyncDirection,
    LocalSyncResult, PlaybookDiffItem, PlaybooksDiffResult, SkillDiffItem, SkillsDiffResult,
};

mod diff_status_tests {
    use super::*;

    #[test]
    fn added_serializes() {
        let json = serde_json::to_string(&DiffStatus::Added).unwrap();
        assert_eq!(json, "\"Added\"");
    }

    #[test]
    fn removed_serializes() {
        let json = serde_json::to_string(&DiffStatus::Removed).unwrap();
        assert_eq!(json, "\"Removed\"");
    }

    #[test]
    fn modified_serializes() {
        let json = serde_json::to_string(&DiffStatus::Modified).unwrap();
        assert_eq!(json, "\"Modified\"");
    }

    #[test]
    fn is_clone() {
        let status = DiffStatus::Added;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn is_copy() {
        let status = DiffStatus::Modified;
        let copied: DiffStatus = status;
        assert_eq!(status, copied);
    }

    #[test]
    fn is_eq() {
        assert_eq!(DiffStatus::Added, DiffStatus::Added);
        assert_ne!(DiffStatus::Added, DiffStatus::Removed);
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", DiffStatus::Modified);
        assert!(debug.contains("Modified"));
    }
}

mod local_sync_direction_tests {
    use super::*;

    #[test]
    fn to_disk_is_distinct() {
        assert_eq!(LocalSyncDirection::ToDisk, LocalSyncDirection::ToDisk);
    }

    #[test]
    fn to_database_is_distinct() {
        assert_eq!(
            LocalSyncDirection::ToDatabase,
            LocalSyncDirection::ToDatabase
        );
    }

    #[test]
    fn directions_are_different() {
        assert_ne!(LocalSyncDirection::ToDisk, LocalSyncDirection::ToDatabase);
    }

    #[test]
    fn is_clone() {
        let dir = LocalSyncDirection::ToDisk;
        let cloned = dir;
        assert_eq!(dir, cloned);
    }

    #[test]
    fn is_copy() {
        let dir = LocalSyncDirection::ToDatabase;
        let copied: LocalSyncDirection = dir;
        assert_eq!(dir, copied);
    }

    #[test]
    fn is_debug() {
        let debug = format!("{:?}", LocalSyncDirection::ToDisk);
        assert!(debug.contains("ToDisk"));
    }
}

mod content_diff_result_tests {
    use super::*;
    use chrono::Utc;

    fn empty_result() -> ContentDiffResult {
        ContentDiffResult {
            source_id: "blog".to_string(),
            added: vec![],
            removed: vec![],
            modified: vec![],
            unchanged: 0,
        }
    }

    fn added_item() -> ContentDiffItem {
        ContentDiffItem {
            slug: "new-post".to_string(),
            source_id: "blog".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash1".to_string()),
            db_hash: None,
            disk_updated_at: Some(Utc::now()),
            db_updated_at: None,
            title: Some("New Post".to_string()),
        }
    }

    #[test]
    fn empty_result_has_no_changes() {
        let result = empty_result();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_with_added_has_changes() {
        let mut result = empty_result();
        result.added.push(added_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_removed_has_changes() {
        let mut result = empty_result();
        result.removed.push(added_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_modified_has_changes() {
        let mut result = empty_result();
        result.modified.push(added_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_unchanged_only_has_no_changes() {
        let mut result = empty_result();
        result.unchanged = 10;
        assert!(!result.has_changes());
    }

    #[test]
    fn default_has_no_changes() {
        let result = ContentDiffResult::default();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_is_serializable() {
        let result = empty_result();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("blog"));
    }

    #[test]
    fn result_is_clone() {
        let result = empty_result();
        let cloned = result.clone();
        assert_eq!(cloned.source_id, result.source_id);
    }

    #[test]
    fn result_is_debug() {
        let result = empty_result();
        let debug = format!("{:?}", result);
        assert!(debug.contains("ContentDiffResult"));
    }
}

mod skills_diff_result_tests {
    use super::*;

    fn empty_result() -> SkillsDiffResult {
        SkillsDiffResult {
            added: vec![],
            removed: vec![],
            modified: vec![],
            unchanged: 0,
        }
    }

    fn skill_item() -> SkillDiffItem {
        SkillDiffItem {
            skill_id: "skill-1".to_string(),
            file_path: "skills/skill.md".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash".to_string()),
            db_hash: None,
            name: Some("Test Skill".to_string()),
        }
    }

    #[test]
    fn empty_result_has_no_changes() {
        let result = empty_result();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_with_added_has_changes() {
        let mut result = empty_result();
        result.added.push(skill_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_removed_has_changes() {
        let mut result = empty_result();
        result.removed.push(skill_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_modified_has_changes() {
        let mut result = empty_result();
        result.modified.push(skill_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_unchanged_only_has_no_changes() {
        let mut result = empty_result();
        result.unchanged = 5;
        assert!(!result.has_changes());
    }

    #[test]
    fn default_has_no_changes() {
        let result = SkillsDiffResult::default();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_is_serializable() {
        let result = empty_result();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("added"));
    }

    #[test]
    fn result_is_clone() {
        let result = empty_result();
        let cloned = result.clone();
        assert_eq!(cloned.unchanged, result.unchanged);
    }

    #[test]
    fn result_is_debug() {
        let result = empty_result();
        let debug = format!("{:?}", result);
        assert!(debug.contains("SkillsDiffResult"));
    }
}

mod playbooks_diff_result_tests {
    use super::*;

    fn empty_result() -> PlaybooksDiffResult {
        PlaybooksDiffResult {
            added: vec![],
            removed: vec![],
            modified: vec![],
            unchanged: 0,
        }
    }

    fn playbook_item() -> PlaybookDiffItem {
        PlaybookDiffItem {
            playbook_id: "pb-1".to_string(),
            file_path: "playbooks/test.md".to_string(),
            category: "automation".to_string(),
            domain: "testing".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash".to_string()),
            db_hash: None,
            name: Some("Test Playbook".to_string()),
        }
    }

    #[test]
    fn empty_result_has_no_changes() {
        let result = empty_result();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_with_added_has_changes() {
        let mut result = empty_result();
        result.added.push(playbook_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_removed_has_changes() {
        let mut result = empty_result();
        result.removed.push(playbook_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_modified_has_changes() {
        let mut result = empty_result();
        result.modified.push(playbook_item());
        assert!(result.has_changes());
    }

    #[test]
    fn result_with_unchanged_only_has_no_changes() {
        let mut result = empty_result();
        result.unchanged = 3;
        assert!(!result.has_changes());
    }

    #[test]
    fn default_has_no_changes() {
        let result = PlaybooksDiffResult::default();
        assert!(!result.has_changes());
    }

    #[test]
    fn result_is_serializable() {
        let result = empty_result();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("added"));
    }

    #[test]
    fn result_is_clone() {
        let result = empty_result();
        let cloned = result.clone();
        assert_eq!(cloned.unchanged, result.unchanged);
    }

    #[test]
    fn result_is_debug() {
        let result = empty_result();
        let debug = format!("{:?}", result);
        assert!(debug.contains("PlaybooksDiffResult"));
    }
}

mod local_sync_result_tests {
    use super::*;

    fn test_result() -> LocalSyncResult {
        LocalSyncResult {
            items_synced: 5,
            items_skipped: 2,
            items_skipped_modified: 1,
            items_deleted: 3,
            errors: vec!["error1".to_string()],
            direction: "to_disk".to_string(),
        }
    }

    #[test]
    fn result_items_synced() {
        let result = test_result();
        assert_eq!(result.items_synced, 5);
    }

    #[test]
    fn result_items_skipped() {
        let result = test_result();
        assert_eq!(result.items_skipped, 2);
    }

    #[test]
    fn result_items_skipped_modified() {
        let result = test_result();
        assert_eq!(result.items_skipped_modified, 1);
    }

    #[test]
    fn result_items_deleted() {
        let result = test_result();
        assert_eq!(result.items_deleted, 3);
    }

    #[test]
    fn result_errors() {
        let result = test_result();
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0], "error1");
    }

    #[test]
    fn result_direction() {
        let result = test_result();
        assert_eq!(result.direction, "to_disk");
    }

    #[test]
    fn default_has_zeros() {
        let result = LocalSyncResult::default();
        assert_eq!(result.items_synced, 0);
        assert_eq!(result.items_skipped, 0);
        assert_eq!(result.items_deleted, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn result_is_serializable() {
        let result = test_result();
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("to_disk"));
    }

    #[test]
    fn result_is_clone() {
        let result = test_result();
        let cloned = result.clone();
        assert_eq!(cloned.items_synced, result.items_synced);
    }

    #[test]
    fn result_is_debug() {
        let result = test_result();
        let debug = format!("{:?}", result);
        assert!(debug.contains("LocalSyncResult"));
    }
}

mod content_hash_tests {
    use super::*;

    #[test]
    fn same_input_same_hash() {
        let hash1 = compute_content_hash("body", "title");
        let hash2 = compute_content_hash("body", "title");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_body_different_hash() {
        let hash1 = compute_content_hash("body1", "title");
        let hash2 = compute_content_hash("body2", "title");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn different_title_different_hash() {
        let hash1 = compute_content_hash("body", "title1");
        let hash2 = compute_content_hash("body", "title2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn empty_inputs_produce_hash() {
        let hash = compute_content_hash("", "");
        assert!(!hash.is_empty());
    }

    #[test]
    fn hash_is_hex_string() {
        let hash = compute_content_hash("test", "title");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_length_is_consistent() {
        let hash1 = compute_content_hash("short", "t");
        let hash2 = compute_content_hash("much longer body content here", "longer title");
        assert_eq!(hash1.len(), hash2.len());
    }

    #[test]
    fn hash_is_sha256_length() {
        let hash = compute_content_hash("test", "title");
        assert_eq!(hash.len(), 64);
    }
}
