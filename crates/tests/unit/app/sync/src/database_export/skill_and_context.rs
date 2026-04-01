//! Tests for SkillExport and ContextExport

use chrono::Utc;
use systemprompt_sync::{ContextExport, SkillExport};

mod skill_export_tests {
    use super::*;

    #[test]
    fn creation_with_tags() {
        let now = Utc::now();
        let skill = SkillExport {
            skill_id: "test_skill".to_string(),
            file_path: "/skills/test-skill/SKILL.md".to_string(),
            name: "Test Skill".to_string(),
            description: "A skill for testing".to_string(),
            instructions: "Follow these instructions".to_string(),
            enabled: true,
            tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
            category_id: Some("skills".to_string()),
            source_id: "skills".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(skill.skill_id, "test_skill");
        assert_eq!(skill.name, "Test Skill");
        assert!(skill.enabled);
        skill.tags.as_ref().expect("Should have tags");
    }

    #[test]
    fn creation_without_tags() {
        let now = Utc::now();
        let skill = SkillExport {
            skill_id: "minimal_skill".to_string(),
            file_path: "/skills/minimal/SKILL.md".to_string(),
            name: "Minimal Skill".to_string(),
            description: "Minimal description".to_string(),
            instructions: "Instructions".to_string(),
            enabled: false,
            tags: None,
            category_id: None,
            source_id: "skills".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert!(skill.tags.is_none());
        assert!(skill.category_id.is_none());
        assert!(!skill.enabled);
    }
}

mod context_export_tests {
    use super::*;

    #[test]
    fn creation_with_session() {
        let now = Utc::now();
        let context = ContextExport {
            context_id: "ctx_123".to_string(),
            user_id: "user_456".to_string(),
            session_id: Some("session_789".to_string()),
            name: "Test Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(context.context_id, "ctx_123");
        assert_eq!(context.user_id, "user_456");
        context.session_id.as_ref().expect("Should have session id");
    }

    #[test]
    fn creation_without_session() {
        let now = Utc::now();
        let context = ContextExport {
            context_id: "ctx_no_session".to_string(),
            user_id: "user_123".to_string(),
            session_id: None,
            name: "No Session Context".to_string(),
            created_at: now,
            updated_at: now,
        };

        assert!(context.session_id.is_none());
    }
}
