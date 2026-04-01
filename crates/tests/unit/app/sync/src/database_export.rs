use chrono::{TimeZone, Utc};
use systemprompt_sync::{ContextExport, DatabaseExport, SkillExport};

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
        skill.tags.expect("skill.tags should be present");
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
        context.session_id.expect("context.session_id should be present");
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

mod database_export_tests {
    use super::*;

    #[test]
    fn full_export() {
        let now = Utc::now();
        let export = DatabaseExport {
            users: vec![],
            skills: vec![SkillExport {
                skill_id: "skill_1".to_string(),
                file_path: "/skills/skill-1/SKILL.md".to_string(),
                name: "Skill".to_string(),
                description: "Description".to_string(),
                instructions: "Instructions".to_string(),
                enabled: true,
                tags: None,
                category_id: None,
                source_id: "skills".to_string(),
                created_at: now,
                updated_at: now,
            }],
            contexts: vec![ContextExport {
                context_id: "ctx_1".to_string(),
                user_id: "user_1".to_string(),
                session_id: None,
                name: "Context".to_string(),
                created_at: now,
                updated_at: now,
            }],
            timestamp: now,
        };

        assert_eq!(export.skills.len(), 1);
        assert_eq!(export.contexts.len(), 1);
    }

    #[test]
    fn empty_export() {
        let export = DatabaseExport {
            users: vec![],
            skills: vec![],
            contexts: vec![],
            timestamp: Utc::now(),
        };

        assert!(export.skills.is_empty());
        assert!(export.contexts.is_empty());
    }

    #[test]
    fn serialization() {
        let now = Utc
            .with_ymd_and_hms(2024, 1, 15, 12, 0, 0)
            .single()
            .expect("valid datetime");
        let export = DatabaseExport {
            users: vec![],
            skills: vec![],
            contexts: vec![],
            timestamp: now,
        };

        let json = serde_json::to_string(&export).expect("serialize database export");
        assert!(json.contains("\"skills\":[]"));
        assert!(json.contains("\"contexts\":[]"));
    }

    #[test]
    fn multiple_skills() {
        let now = Utc::now();
        let skills: Vec<SkillExport> = (0..100)
            .map(|i| SkillExport {
                skill_id: format!("skill_{}", i),
                file_path: format!("/skills/skill-{}/SKILL.md", i),
                name: format!("Skill {}", i),
                description: format!("Description {}", i),
                instructions: format!("Instructions {}", i),
                enabled: true,
                tags: None,
                category_id: None,
                source_id: "skills".to_string(),
                created_at: now,
                updated_at: now,
            })
            .collect();

        let export = DatabaseExport {
            users: vec![],
            skills,
            contexts: vec![],
            timestamp: now,
        };

        assert_eq!(export.skills.len(), 100);
    }

    #[test]
    fn roundtrip() {
        let now = Utc::now();
        let original = DatabaseExport {
            users: vec![],
            skills: vec![SkillExport {
                skill_id: "test_skill".to_string(),
                file_path: "/skills/test/SKILL.md".to_string(),
                name: "Test".to_string(),
                description: "Description".to_string(),
                instructions: "Instructions".to_string(),
                enabled: true,
                tags: None,
                category_id: None,
                source_id: "skills".to_string(),
                created_at: now,
                updated_at: now,
            }],
            contexts: vec![],
            timestamp: now,
        };

        let json = serde_json::to_string(&original).expect("serialize database export");
        let restored: DatabaseExport =
            serde_json::from_str(&json).expect("deserialize database export");

        assert_eq!(original.skills.len(), restored.skills.len());
        assert_eq!(original.skills[0].name, restored.skills[0].name);
    }

    #[test]
    fn with_users() {
        use systemprompt_sync::database::UserExport;

        let now = Utc::now();
        let export = DatabaseExport {
            users: vec![
                UserExport {
                    id: "user_1".to_string(),
                    name: "user1".to_string(),
                    email: "user1@example.com".to_string(),
                    full_name: Some("User One".to_string()),
                    display_name: None,
                    status: "active".to_string(),
                    email_verified: true,
                    roles: vec!["admin".to_string()],
                    is_bot: false,
                    is_scanner: false,
                    avatar_url: None,
                    created_at: now,
                    updated_at: now,
                },
                UserExport {
                    id: "user_2".to_string(),
                    name: "user2".to_string(),
                    email: "user2@example.com".to_string(),
                    full_name: None,
                    display_name: Some("U2".to_string()),
                    status: "pending".to_string(),
                    email_verified: false,
                    roles: vec![],
                    is_bot: true,
                    is_scanner: false,
                    avatar_url: Some("https://example.com/u2.png".to_string()),
                    created_at: now,
                    updated_at: now,
                },
            ],
            skills: vec![],
            contexts: vec![],
            timestamp: now,
        };

        assert_eq!(export.users.len(), 2);
        assert_eq!(export.users[0].name, "user1");
        assert_eq!(export.users[1].name, "user2");
    }

    #[test]
    fn full_roundtrip() {
        use systemprompt_sync::database::UserExport;

        let now = Utc::now();
        let original = DatabaseExport {
            users: vec![UserExport {
                id: "u1".to_string(),
                name: "name".to_string(),
                email: "email@test.com".to_string(),
                full_name: Some("Full Name".to_string()),
                display_name: Some("Display".to_string()),
                status: "active".to_string(),
                email_verified: true,
                roles: vec!["role1".to_string(), "role2".to_string()],
                is_bot: false,
                is_scanner: true,
                avatar_url: Some("https://avatar.url".to_string()),
                created_at: now,
                updated_at: now,
            }],
            skills: vec![SkillExport {
                skill_id: "sk1".to_string(),
                file_path: "/skills/sk1/SKILL.md".to_string(),
                name: "Skill".to_string(),
                description: "Desc".to_string(),
                instructions: "Instr".to_string(),
                enabled: true,
                tags: Some(vec!["t1".to_string()]),
                category_id: Some("cat".to_string()),
                source_id: "skills".to_string(),
                created_at: now,
                updated_at: now,
            }],
            contexts: vec![ContextExport {
                context_id: "ctx1".to_string(),
                user_id: "u1".to_string(),
                session_id: Some("sess1".to_string()),
                name: "Context".to_string(),
                created_at: now,
                updated_at: now,
            }],
            timestamp: now,
        };

        let json = serde_json::to_string(&original).expect("serialize full database export");
        let restored: DatabaseExport =
            serde_json::from_str(&json).expect("deserialize full database export");

        assert_eq!(original.users.len(), restored.users.len());
        assert_eq!(original.skills.len(), restored.skills.len());
        assert_eq!(original.contexts.len(), restored.contexts.len());
    }
}

mod user_export_tests {
    use chrono::Utc;
    use systemprompt_sync::database::UserExport;

    #[test]
    fn creation() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_123".to_string(),
            name: "testuser".to_string(),
            email: "test@example.com".to_string(),
            full_name: Some("Test User".to_string()),
            display_name: Some("Test".to_string()),
            status: "active".to_string(),
            email_verified: true,
            roles: vec!["user".to_string(), "admin".to_string()],
            is_bot: false,
            is_scanner: false,
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            created_at: now,
            updated_at: now,
        };

        assert_eq!(user.id, "user_123");
        assert!(user.email_verified);
        assert_eq!(user.roles.len(), 2);
    }

    #[test]
    fn minimal() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_min".to_string(),
            name: "minimal".to_string(),
            email: "min@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: "pending".to_string(),
            email_verified: false,
            roles: vec![],
            is_bot: true,
            is_scanner: true,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        };

        assert!(user.full_name.is_none());
        assert!(user.roles.is_empty());
        assert!(user.is_bot);
    }

    #[test]
    fn serialization() {
        let now = Utc::now();
        let user = UserExport {
            id: "user_ser".to_string(),
            name: "seruser".to_string(),
            email: "ser@example.com".to_string(),
            full_name: None,
            display_name: None,
            status: "active".to_string(),
            email_verified: true,
            roles: vec!["user".to_string()],
            is_bot: false,
            is_scanner: false,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&user).expect("serialize user export");
        assert!(json.contains("\"id\":\"user_ser\""));
        assert!(json.contains("\"email_verified\":true"));
    }

    #[test]
    fn roundtrip() {
        let now = Utc::now();
        let original = UserExport {
            id: "user_rt".to_string(),
            name: "roundtrip".to_string(),
            email: "rt@example.com".to_string(),
            full_name: Some("Round Trip".to_string()),
            display_name: Some("RT".to_string()),
            status: "active".to_string(),
            email_verified: true,
            roles: vec!["admin".to_string()],
            is_bot: false,
            is_scanner: false,
            avatar_url: Some("https://example.com/rt.png".to_string()),
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&original).expect("serialize user export");
        let restored: UserExport = serde_json::from_str(&json).expect("deserialize user export");

        assert_eq!(original.id, restored.id);
        assert_eq!(original.full_name, restored.full_name);
    }
}

mod import_result_tests {
    use systemprompt_sync::database::ImportResult;

    #[test]
    fn creation() {
        let result = ImportResult {
            created: 10,
            updated: 5,
            skipped: 2,
        };

        assert_eq!(result.created, 10);
        assert_eq!(result.updated, 5);
        assert_eq!(result.skipped, 2);
    }

    #[test]
    fn zero_values() {
        let result = ImportResult {
            created: 0,
            updated: 0,
            skipped: 0,
        };

        assert_eq!(result.created, 0);
    }

    #[test]
    fn serialization() {
        let result = ImportResult {
            created: 100,
            updated: 50,
            skipped: 10,
        };

        let json = serde_json::to_string(&result).expect("serialize import result");
        assert!(json.contains("\"created\":100"));
    }

    #[test]
    fn roundtrip() {
        let original = ImportResult {
            created: 25,
            updated: 15,
            skipped: 5,
        };

        let json = serde_json::to_string(&original).expect("serialize import result");
        let restored: ImportResult = serde_json::from_str(&json).expect("deserialize import result");

        assert_eq!(original.created, restored.created);
        assert_eq!(original.updated, restored.updated);
        assert_eq!(original.skipped, restored.skipped);
    }

    #[test]
    fn copy() {
        let result = ImportResult {
            created: 1,
            updated: 2,
            skipped: 3,
        };

        let copied = result;
        assert_eq!(result.created, copied.created);
    }
}
