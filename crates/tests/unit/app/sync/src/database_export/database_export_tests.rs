//! Tests for DatabaseExport struct

use chrono::{TimeZone, Utc};
use systemprompt_sync::{ContextExport, DatabaseExport, SkillExport};

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
