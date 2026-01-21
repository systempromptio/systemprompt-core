//! Unit tests for paper models
//!
//! Tests cover:
//! - PaperSection struct and defaults
//! - PaperMetadata struct and defaults

use systemprompt_content::models::{PaperMetadata, PaperSection};

// ============================================================================
// PaperSection Tests
// ============================================================================

#[test]
fn test_paper_section_default() {
    let section = PaperSection::default();
    assert!(section.id.is_empty());
    assert!(section.title.is_empty());
    assert!(section.file.is_none());
    assert!(section.image.is_none());
    assert!(section.image_alt.is_none());
    // Note: Default derive uses String::default() (empty),
    // while serde's default_image_position is only used during deserialization
    assert!(section.image_position.is_empty());
}

#[test]
fn test_paper_section_deserialization_minimal() {
    let yaml = r#"
id: section-1
title: Introduction
"#;
    let section: PaperSection = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(section.id, "section-1");
    assert_eq!(section.title, "Introduction");
    assert!(section.file.is_none());
    assert!(section.image.is_none());
    assert!(section.image_alt.is_none());
    assert_eq!(section.image_position, "right");
}

#[test]
fn test_paper_section_deserialization_full() {
    let yaml = r#"
id: section-2
title: Chapter One
file: chapters/chapter1.md
image: /images/chapter1.png
image_alt: Chapter 1 illustration
image_position: left
"#;
    let section: PaperSection = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(section.id, "section-2");
    assert_eq!(section.title, "Chapter One");
    assert_eq!(section.file, Some("chapters/chapter1.md".to_string()));
    assert_eq!(section.image, Some("/images/chapter1.png".to_string()));
    assert_eq!(section.image_alt, Some("Chapter 1 illustration".to_string()));
    assert_eq!(section.image_position, "left");
}

#[test]
fn test_paper_section_serialization() {
    let section = PaperSection {
        id: "test-id".to_string(),
        title: "Test Title".to_string(),
        file: Some("test.md".to_string()),
        image: None,
        image_alt: None,
        image_position: "center".to_string(),
    };

    let json = serde_json::to_string(&section).unwrap();
    assert!(json.contains("\"id\":\"test-id\""));
    assert!(json.contains("\"title\":\"Test Title\""));
    assert!(json.contains("\"file\":\"test.md\""));
    assert!(json.contains("\"image_position\":\"center\""));
}

#[test]
fn test_paper_section_clone() {
    let section = PaperSection {
        id: "clone-test".to_string(),
        title: "Clone Test".to_string(),
        file: Some("file.md".to_string()),
        image: Some("/img.png".to_string()),
        image_alt: Some("Alt text".to_string()),
        image_position: "right".to_string(),
    };

    let cloned = section.clone();
    assert_eq!(cloned.id, section.id);
    assert_eq!(cloned.title, section.title);
    assert_eq!(cloned.file, section.file);
    assert_eq!(cloned.image, section.image);
    assert_eq!(cloned.image_alt, section.image_alt);
    assert_eq!(cloned.image_position, section.image_position);
}

// ============================================================================
// PaperMetadata Tests
// ============================================================================

#[test]
fn test_paper_metadata_default() {
    let metadata = PaperMetadata::default();
    assert!(metadata.hero_image.is_none());
    assert!(metadata.hero_alt.is_none());
    assert!(metadata.sections.is_empty());
    assert!(!metadata.toc);
    assert!(metadata.chapters_path.is_none());
}

#[test]
fn test_paper_metadata_deserialization_minimal() {
    let yaml = r#"
sections: []
"#;
    let metadata: PaperMetadata = serde_yaml::from_str(yaml).unwrap();
    assert!(metadata.hero_image.is_none());
    assert!(metadata.sections.is_empty());
    assert!(!metadata.toc);
}

#[test]
fn test_paper_metadata_deserialization_full() {
    let yaml = r#"
hero_image: /images/hero.png
hero_alt: Hero image description
toc: true
chapters_path: /content/chapters
sections:
  - id: intro
    title: Introduction
  - id: chapter1
    title: Chapter 1
    file: ch1.md
"#;
    let metadata: PaperMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.hero_image, Some("/images/hero.png".to_string()));
    assert_eq!(metadata.hero_alt, Some("Hero image description".to_string()));
    assert!(metadata.toc);
    assert_eq!(metadata.chapters_path, Some("/content/chapters".to_string()));
    assert_eq!(metadata.sections.len(), 2);
    assert_eq!(metadata.sections[0].id, "intro");
    assert_eq!(metadata.sections[0].title, "Introduction");
    assert_eq!(metadata.sections[1].id, "chapter1");
    assert_eq!(metadata.sections[1].file, Some("ch1.md".to_string()));
}

#[test]
fn test_paper_metadata_with_toc_false() {
    let yaml = r#"
toc: false
sections:
  - id: sec1
    title: Section 1
"#;
    let metadata: PaperMetadata = serde_yaml::from_str(yaml).unwrap();
    assert!(!metadata.toc);
}

#[test]
fn test_paper_metadata_serialization() {
    let metadata = PaperMetadata {
        hero_image: Some("/hero.jpg".to_string()),
        hero_alt: Some("Hero".to_string()),
        sections: vec![PaperSection {
            id: "s1".to_string(),
            title: "Section 1".to_string(),
            file: None,
            image: None,
            image_alt: None,
            image_position: "right".to_string(),
        }],
        toc: true,
        chapters_path: Some("/chapters".to_string()),
    };

    let json = serde_json::to_string(&metadata).unwrap();
    assert!(json.contains("\"hero_image\":\"/hero.jpg\""));
    assert!(json.contains("\"toc\":true"));
    assert!(json.contains("\"chapters_path\":\"/chapters\""));
}

#[test]
fn test_paper_metadata_clone() {
    let metadata = PaperMetadata {
        hero_image: Some("/hero.png".to_string()),
        hero_alt: None,
        sections: vec![PaperSection::default()],
        toc: true,
        chapters_path: Some("/path".to_string()),
    };

    let cloned = metadata.clone();
    assert_eq!(cloned.hero_image, metadata.hero_image);
    assert_eq!(cloned.toc, metadata.toc);
    assert_eq!(cloned.sections.len(), metadata.sections.len());
}

#[test]
fn test_paper_metadata_multiple_sections() {
    let yaml = r#"
sections:
  - id: s1
    title: First
  - id: s2
    title: Second
  - id: s3
    title: Third
    file: third.md
    image: /img/third.png
    image_alt: Third section
    image_position: left
"#;
    let metadata: PaperMetadata = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(metadata.sections.len(), 3);
    assert_eq!(metadata.sections[2].image_position, "left");
}
