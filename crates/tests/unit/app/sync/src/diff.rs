use chrono::Utc;
use systemprompt_identifiers::{SkillId, SourceId};
use systemprompt_sync::{
    ContentDiffItem, ContentDiffResult, DiffStatus, DiskContent, DiskSkill, LocalSyncDirection,
    LocalSyncResult, SkillDiffItem, SkillsDiffResult,
};

mod local_sync_direction_tests {
    use super::*;

    #[test]
    fn debug_format() {
        assert!(format!("{:?}", LocalSyncDirection::ToDisk).contains("ToDisk"));
        assert!(format!("{:?}", LocalSyncDirection::ToDatabase).contains("ToDatabase"));
    }

    #[test]
    fn copy() {
        let direction = LocalSyncDirection::ToDisk;
        let copied = direction;
        assert_eq!(direction, copied);
    }
}

mod diff_status_tests {
    use super::*;

    #[test]
    fn serialization() {
        assert_eq!(serde_json::to_string(&DiffStatus::Added).unwrap(), "\"Added\"");
        assert_eq!(serde_json::to_string(&DiffStatus::Removed).unwrap(), "\"Removed\"");
        assert_eq!(serde_json::to_string(&DiffStatus::Modified).unwrap(), "\"Modified\"");
    }
}

mod content_diff_item_tests {
    use super::*;

    #[test]
    fn added() {
        let item = ContentDiffItem {
            slug: "new-article".to_string(),
            source_id: SourceId::new("blog"),
            status: DiffStatus::Added,
            disk_hash: Some("abc123".to_string()),
            db_hash: None,
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("New Article".to_string()),
        };
        assert_eq!(item.status, DiffStatus::Added);
        item.disk_hash.expect("item.disk_hash should be present");
        assert!(item.db_hash.is_none());
    }

    #[test]
    fn removed() {
        let now = Utc::now();
        let item = ContentDiffItem {
            slug: "old-article".to_string(),
            source_id: SourceId::new("blog"),
            status: DiffStatus::Removed,
            disk_hash: None,
            db_hash: Some("def456".to_string()),
            disk_updated_at: None,
            db_updated_at: Some(now),
            title: Some("Old Article".to_string()),
        };
        assert_eq!(item.status, DiffStatus::Removed);
        item.db_hash.expect("item.db_hash should be present");
    }

    #[test]
    fn modified() {
        let item = ContentDiffItem {
            slug: "updated-article".to_string(),
            source_id: SourceId::new("blog"),
            status: DiffStatus::Modified,
            disk_hash: Some("new_hash".to_string()),
            db_hash: Some("old_hash".to_string()),
            disk_updated_at: None,
            db_updated_at: None,
            title: Some("Updated Article".to_string()),
        };
        assert_ne!(item.disk_hash, item.db_hash);
    }
}

mod skill_diff_item_tests {
    use super::*;

    #[test]
    fn creation() {
        let item = SkillDiffItem {
            skill_id: SkillId::new("new_skill"),
            file_path: "/skills/new-skill/SKILL.md".to_string(),
            status: DiffStatus::Added,
            disk_hash: Some("hash123".to_string()),
            db_hash: None,
            name: Some("New Skill".to_string()),
        };
        assert_eq!(item.skill_id, "new_skill");
        assert_eq!(item.status, DiffStatus::Added);
    }
}

mod content_diff_result_tests {
    use super::*;

    #[test]
    fn no_changes() {
        let result = ContentDiffResult {
            source_id: SourceId::new("test-source"),
            added: vec![],
            removed: vec![],
            modified: vec![],
            unchanged: 5,
        };
        assert!(!result.has_changes());
    }

    #[test]
    fn with_additions() {
        let result = ContentDiffResult {
            source_id: SourceId::new("test-source"),
            added: vec![ContentDiffItem {
                slug: "new".to_string(),
                source_id: SourceId::new("test"),
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
    }

    #[test]
    fn serialization() {
        let result = ContentDiffResult {
            source_id: SourceId::new("blog"),
            added: vec![ContentDiffItem {
                slug: "new-post".to_string(),
                source_id: SourceId::new("blog"),
                status: DiffStatus::Added,
                disk_hash: Some("hash1".to_string()),
                db_hash: None,
                disk_updated_at: None,
                db_updated_at: None,
                title: Some("New Post".to_string()),
            }],
            removed: vec![],
            modified: vec![],
            unchanged: 3,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("new-post"));
    }

    #[test]
    fn default() {
        let result = ContentDiffResult::default();
        assert!(!result.has_changes());
        assert_eq!(result.unchanged, 0);
    }
}

mod skills_diff_result_tests {
    use super::*;

    #[test]
    fn no_changes() {
        let result = SkillsDiffResult::default();
        assert!(!result.has_changes());
    }

    #[test]
    fn with_changes() {
        let result = SkillsDiffResult {
            added: vec![SkillDiffItem {
                skill_id: SkillId::new("skill1"),
                file_path: "/skills/skill1/SKILL.md".to_string(),
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
    }

    #[test]
    fn serialization() {
        let result = SkillsDiffResult {
            added: vec![SkillDiffItem {
                skill_id: SkillId::new("new_skill"),
                file_path: "/skills/new/SKILL.md".to_string(),
                status: DiffStatus::Added,
                disk_hash: Some("hash".to_string()),
                db_hash: None,
                name: Some("New Skill".to_string()),
            }],
            removed: vec![],
            modified: vec![],
            unchanged: 10,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        assert!(json.contains("new_skill"));
    }
}

mod local_sync_result_tests {
    use super::*;

    #[test]
    fn default() {
        let result = LocalSyncResult::default();
        assert_eq!(result.items_synced, 0);
        assert_eq!(result.direction, LocalSyncDirection::ToDisk);
    }

    #[test]
    fn with_values() {
        let result = LocalSyncResult {
            items_synced: 10,
            items_skipped: 2,
            items_skipped_modified: 0,
            items_deleted: 1,
            errors: vec!["Error 1".to_string()],
            direction: LocalSyncDirection::ToDisk,
        };
        assert_eq!(result.items_synced, 10);
        assert_eq!(result.errors.len(), 1);
    }
}

mod disk_model_tests {
    use super::*;

    #[test]
    fn disk_content() {
        let content = DiskContent {
            slug: "test-article".to_string(),
            title: "Test Article".to_string(),
            body: "Article body content".to_string(),
        };
        assert_eq!(content.slug, "test-article");
    }

    #[test]
    fn disk_skill() {
        let skill = DiskSkill {
            skill_id: SkillId::new("test_skill"),
            name: "Test Skill".to_string(),
            description: "A test skill".to_string(),
            instructions: "Do something useful".to_string(),
            file_path: "/skills/test-skill/SKILL.md".to_string(),
        };
        assert_eq!(skill.skill_id, "test_skill");
    }
}

mod content_diff_entry_tests {
    use std::path::PathBuf;
    use systemprompt_identifiers::SourceId;
    use systemprompt_sync::{ContentDiffEntry, ContentDiffResult};

    #[test]
    fn creation() {
        let diff = ContentDiffResult {
            source_id: SourceId::new("blog"),
            added: vec![],
            removed: vec![],
            modified: vec![],
            unchanged: 5,
        };
        let entry = ContentDiffEntry {
            name: "blog".to_string(),
            source_id: "content_blog".to_string(),
            category_id: "blog_category".to_string(),
            path: PathBuf::from("/content/blog"),
            allowed_content_types: vec!["article".to_string(), "post".to_string()],
            diff,
        };
        assert_eq!(entry.name, "blog");
        assert_eq!(entry.allowed_content_types.len(), 2);
    }

    #[test]
    fn debug() {
        let entry = ContentDiffEntry {
            name: "docs".to_string(),
            source_id: "docs_source".to_string(),
            category_id: "docs_cat".to_string(),
            path: PathBuf::from("/content/docs"),
            allowed_content_types: vec!["doc".to_string()],
            diff: ContentDiffResult::default(),
        };
        let debug_str = format!("{:?}", entry);
        assert!(debug_str.contains("ContentDiffEntry"));
    }
}
