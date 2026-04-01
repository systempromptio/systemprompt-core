use chrono::{TimeZone, Utc};
use std::fs;
use systemprompt_sync::{
    compute_content_hash, escape_yaml, export_content_to_file, export_skill_to_disk,
    generate_content_markdown, generate_skill_config, generate_skill_markdown,
};
use tempfile::TempDir;

mod content_hash_tests {
    use super::*;

    #[test]
    fn basic() {
        let hash = compute_content_hash("This is the content body", "Test Title");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn consistency() {
        let hash1 = compute_content_hash("Same content", "Same title");
        let hash2 = compute_content_hash("Same content", "Same title");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_content() {
        let hash1 = compute_content_hash("Content A", "Same title");
        let hash2 = compute_content_hash("Content B", "Same title");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn different_title() {
        let hash1 = compute_content_hash("Same content", "Title A");
        let hash2 = compute_content_hash("Same content", "Title B");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn empty() {
        let hash = compute_content_hash("", "");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn whitespace_matters() {
        let hash1 = compute_content_hash("test", "title");
        let hash2 = compute_content_hash("test ", "title");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn order_matters() {
        let hash1 = compute_content_hash("body", "title");
        let hash2 = compute_content_hash("title", "body");
        assert_ne!(hash1, hash2);
    }
}

mod escape_yaml_tests {
    use super::*;

    #[test]
    fn plain_string() { assert_eq!(escape_yaml("Simple text"), "Simple text"); }

    #[test]
    fn backslash() { assert_eq!(escape_yaml(r"Path\to\file"), r"Path\\to\\file"); }

    #[test]
    fn quotes() { assert_eq!(escape_yaml(r#"Say "hello""#), r#"Say \"hello\""#); }

    #[test]
    fn newlines() { assert_eq!(escape_yaml("Line1\nLine2"), r"Line1\nLine2"); }

    #[test]
    fn combined() {
        assert_eq!(escape_yaml("Path\\to\\file \"with\nnewline\""), r#"Path\\to\\file \"with\nnewline\""#);
    }

    #[test]
    fn empty() { assert_eq!(escape_yaml(""), ""); }

    #[test]
    fn multiple_escapes() { assert_eq!(escape_yaml("a\\b\"c\nd"), r#"a\\b\"c\nd"#); }
}

mod skill_generation_tests {
    use super::*;
    use systemprompt_agent::models::Skill;
    use systemprompt_identifiers::{CategoryId, SkillId, SourceId};

    #[test]
    fn markdown_structure() {
        let skill = Skill {
            skill_id: SkillId::new("test_skill"),
            file_path: "/skills/test-skill/SKILL.md".to_string(),
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
        assert!(markdown.contains("description: \"A test skill description\""));
        assert!(markdown.contains("Follow these instructions carefully."));
    }

    #[test]
    fn config_structure() {
        let skill = Skill {
            skill_id: SkillId::new("config_test"),
            file_path: "/skills/config-test/SKILL.md".to_string(),
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
        assert!(config.contains("enabled: true"));
    }

    #[test]
    fn config_empty_tags() {
        let skill = Skill {
            skill_id: SkillId::new("no_tags"),
            file_path: "/skills/no-tags/SKILL.md".to_string(),
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
        assert!(config.contains("enabled: false"));
    }

    #[test]
    fn export_to_disk_creates_files() {
        let temp_dir = TempDir::new().expect("create temp directory");
        let skill = Skill {
            skill_id: SkillId::new("export_test"),
            file_path: "/skills/export-test/SKILL.md".to_string(),
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
        result.expect("result should succeed");
        let skill_dir = temp_dir.path().join("export-test");
        assert!(skill_dir.exists());
        assert!(skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("config.yaml").exists());
    }

    #[test]
    fn export_underscore_to_dash() {
        let temp_dir = TempDir::new().expect("create temp directory");
        let skill = Skill {
            skill_id: SkillId::new("my_complex_skill_name"),
            file_path: "/skills/my-complex-skill-name/SKILL.md".to_string(),
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
        result.expect("result should succeed");
        assert!(temp_dir.path().join("my-complex-skill-name").exists());
    }
}

mod content_generation_tests {
    use super::*;
    use systemprompt_content::models::Content;
    use systemprompt_identifiers::{ContentId, SourceId};

    #[test]
    fn markdown_structure() {
        let content = Content {
            id: ContentId::new("test-id"),
            slug: "test-article".to_string(),
            title: "Test Article".to_string(),
            description: "Article description".to_string(),
            body: "Article body content goes here.".to_string(),
            author: "Test Author".to_string(),
            published_at: Utc.with_ymd_and_hms(2024, 6, 15, 0, 0, 0).single().expect("valid date"),
            keywords: "test, article".to_string(),
            kind: "article".to_string(),
            image: Some("cover.jpg".to_string()),
            category_id: None,
            source_id: SourceId::new("blog"),
            version_hash: "hash123".to_string(),
            public: true,
            links: serde_json::json!([]),
            updated_at: Utc.with_ymd_and_hms(2024, 7, 20, 0, 0, 0).single().expect("valid date"),
        };
        let markdown = generate_content_markdown(&content);
        assert!(markdown.starts_with("---\n"));
        assert!(markdown.contains("title: \"Test Article\""));
        assert!(markdown.contains("Article body content goes here."));
    }

    #[test]
    fn markdown_no_image() {
        let content = Content {
            id: ContentId::new("no-image"),
            slug: "no-image".to_string(),
            title: "No Image".to_string(),
            description: "No image".to_string(),
            body: "Body".to_string(),
            author: "Author".to_string(),
            published_at: Utc::now(),
            keywords: String::new(),
            kind: "article".to_string(),
            image: None,
            category_id: None,
            source_id: SourceId::new("blog"),
            version_hash: "hash".to_string(),
            public: true,
            links: serde_json::json!([]),
            updated_at: Utc::now(),
        };
        let markdown = generate_content_markdown(&content);
        assert!(markdown.contains("image: \"\""));
    }

    #[test]
    fn export_to_file_docs() {
        let temp_dir = TempDir::new().expect("create temp directory");
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
            public: true,
            links: serde_json::json!([]),
            updated_at: Utc::now(),
        };
        let result = export_content_to_file(&content, temp_dir.path(), "docs");
        result.expect("result should succeed");
        let file_path = temp_dir.path().join("getting-started.md");
        assert!(file_path.exists());
    }

    #[test]
    fn export_to_file_blog_creates_directory() {
        let temp_dir = TempDir::new().expect("create temp directory");
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
            public: true,
            links: serde_json::json!([]),
            updated_at: Utc::now(),
        };
        let result = export_content_to_file(&content, temp_dir.path(), "blog");
        result.expect("result should succeed");
        assert!(temp_dir.path().join("my-blog-post").join("index.md").exists());
    }
}
