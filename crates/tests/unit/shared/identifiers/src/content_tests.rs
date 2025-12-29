//! Unit tests for content-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{
    SkillId, SourceId, CategoryId, ContentId, TagId, FileId, ToDbValue, DbValue
};

// ============================================================================
// SkillId Tests
// ============================================================================

#[test]
fn test_skill_id_new() {
    let id = SkillId::new("skill-123");
    assert_eq!(id.as_str(), "skill-123");
}

#[test]
fn test_skill_id_generate() {
    let id = SkillId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_skill_id_generate_unique() {
    let id1 = SkillId::generate();
    let id2 = SkillId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_skill_id_display() {
    let id = SkillId::new("display-skill");
    assert_eq!(format!("{}", id), "display-skill");
}

#[test]
fn test_skill_id_from_string() {
    let id: SkillId = String::from("from-string-skill").into();
    assert_eq!(id.as_str(), "from-string-skill");
}

#[test]
fn test_skill_id_from_str() {
    let id: SkillId = "from-str-skill".into();
    assert_eq!(id.as_str(), "from-str-skill");
}

#[test]
fn test_skill_id_as_ref() {
    let id = SkillId::new("as-ref-skill");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-skill");
}

#[test]
fn test_skill_id_clone_and_eq() {
    let id1 = SkillId::new("clone-skill");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_skill_id_hash() {
    let id1 = SkillId::new("hash-skill");
    let id2 = SkillId::new("hash-skill");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_skill_id_serialize_json() {
    let id = SkillId::new("serialize-skill");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-skill\"");
}

#[test]
fn test_skill_id_deserialize_json() {
    let id: SkillId = serde_json::from_str("\"deserialize-skill\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-skill");
}

#[test]
fn test_skill_id_to_db_value() {
    let id = SkillId::new("db-value-skill");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-skill"));
}

// ============================================================================
// SourceId Tests
// ============================================================================

#[test]
fn test_source_id_new() {
    let id = SourceId::new("source-123");
    assert_eq!(id.as_str(), "source-123");
}

#[test]
fn test_source_id_display() {
    let id = SourceId::new("display-source");
    assert_eq!(format!("{}", id), "display-source");
}

#[test]
fn test_source_id_from_string() {
    let id: SourceId = String::from("from-string-source").into();
    assert_eq!(id.as_str(), "from-string-source");
}

#[test]
fn test_source_id_from_str() {
    let id: SourceId = "from-str-source".into();
    assert_eq!(id.as_str(), "from-str-source");
}

#[test]
fn test_source_id_as_ref() {
    let id = SourceId::new("as-ref-source");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-source");
}

#[test]
fn test_source_id_clone_and_eq() {
    let id1 = SourceId::new("clone-source");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_source_id_hash() {
    let id1 = SourceId::new("hash-source");
    let id2 = SourceId::new("hash-source");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_source_id_serialize_json() {
    let id = SourceId::new("serialize-source");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-source\"");
}

#[test]
fn test_source_id_to_db_value() {
    let id = SourceId::new("db-value-source");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-source"));
}

// ============================================================================
// CategoryId Tests
// ============================================================================

#[test]
fn test_category_id_new() {
    let id = CategoryId::new("category-123");
    assert_eq!(id.as_str(), "category-123");
}

#[test]
fn test_category_id_display() {
    let id = CategoryId::new("display-category");
    assert_eq!(format!("{}", id), "display-category");
}

#[test]
fn test_category_id_from_string() {
    let id: CategoryId = String::from("from-string-category").into();
    assert_eq!(id.as_str(), "from-string-category");
}

#[test]
fn test_category_id_from_str() {
    let id: CategoryId = "from-str-category".into();
    assert_eq!(id.as_str(), "from-str-category");
}

#[test]
fn test_category_id_as_ref() {
    let id = CategoryId::new("as-ref-category");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-category");
}

#[test]
fn test_category_id_clone_and_eq() {
    let id1 = CategoryId::new("clone-category");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_category_id_hash() {
    let id1 = CategoryId::new("hash-category");
    let id2 = CategoryId::new("hash-category");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_category_id_serialize_json() {
    let id = CategoryId::new("serialize-category");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-category\"");
}

#[test]
fn test_category_id_to_db_value() {
    let id = CategoryId::new("db-value-category");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-category"));
}

// ============================================================================
// ContentId Tests
// ============================================================================

#[test]
fn test_content_id_new() {
    let id = ContentId::new("content-123");
    assert_eq!(id.as_str(), "content-123");
}

#[test]
fn test_content_id_display() {
    let id = ContentId::new("display-content");
    assert_eq!(format!("{}", id), "display-content");
}

#[test]
fn test_content_id_from_string() {
    let id: ContentId = String::from("from-string-content").into();
    assert_eq!(id.as_str(), "from-string-content");
}

#[test]
fn test_content_id_from_str() {
    let id: ContentId = "from-str-content".into();
    assert_eq!(id.as_str(), "from-str-content");
}

#[test]
fn test_content_id_as_ref() {
    let id = ContentId::new("as-ref-content");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-content");
}

#[test]
fn test_content_id_clone_and_eq() {
    let id1 = ContentId::new("clone-content");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_content_id_hash() {
    let id1 = ContentId::new("hash-content");
    let id2 = ContentId::new("hash-content");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_content_id_serialize_json() {
    let id = ContentId::new("serialize-content");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-content\"");
}

#[test]
fn test_content_id_to_db_value() {
    let id = ContentId::new("db-value-content");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-content"));
}

// ============================================================================
// TagId Tests
// ============================================================================

#[test]
fn test_tag_id_new() {
    let id = TagId::new("tag-123");
    assert_eq!(id.as_str(), "tag-123");
}

#[test]
fn test_tag_id_display() {
    let id = TagId::new("display-tag");
    assert_eq!(format!("{}", id), "display-tag");
}

#[test]
fn test_tag_id_from_string() {
    let id: TagId = String::from("from-string-tag").into();
    assert_eq!(id.as_str(), "from-string-tag");
}

#[test]
fn test_tag_id_from_str() {
    let id: TagId = "from-str-tag".into();
    assert_eq!(id.as_str(), "from-str-tag");
}

#[test]
fn test_tag_id_as_ref() {
    let id = TagId::new("as-ref-tag");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-tag");
}

#[test]
fn test_tag_id_clone_and_eq() {
    let id1 = TagId::new("clone-tag");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_tag_id_hash() {
    let id1 = TagId::new("hash-tag");
    let id2 = TagId::new("hash-tag");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_tag_id_serialize_json() {
    let id = TagId::new("serialize-tag");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-tag\"");
}

#[test]
fn test_tag_id_to_db_value() {
    let id = TagId::new("db-value-tag");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-tag"));
}

// ============================================================================
// FileId Tests
// ============================================================================

#[test]
fn test_file_id_new() {
    let id = FileId::new("file-123");
    assert_eq!(id.as_str(), "file-123");
}

#[test]
fn test_file_id_generate() {
    let id = FileId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_file_id_generate_unique() {
    let id1 = FileId::generate();
    let id2 = FileId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_file_id_display() {
    let id = FileId::new("display-file");
    assert_eq!(format!("{}", id), "display-file");
}

#[test]
fn test_file_id_from_string() {
    let id: FileId = String::from("from-string-file").into();
    assert_eq!(id.as_str(), "from-string-file");
}

#[test]
fn test_file_id_from_str() {
    let id: FileId = "from-str-file".into();
    assert_eq!(id.as_str(), "from-str-file");
}

#[test]
fn test_file_id_as_ref() {
    let id = FileId::new("as-ref-file");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-file");
}

#[test]
fn test_file_id_clone_and_eq() {
    let id1 = FileId::new("clone-file");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_file_id_hash() {
    let id1 = FileId::new("hash-file");
    let id2 = FileId::new("hash-file");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_file_id_serialize_json() {
    let id = FileId::new("serialize-file");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-file\"");
}

#[test]
fn test_file_id_deserialize_json() {
    let id: FileId = serde_json::from_str("\"deserialize-file\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-file");
}

#[test]
fn test_file_id_to_db_value() {
    let id = FileId::new("db-value-file");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-file"));
}
