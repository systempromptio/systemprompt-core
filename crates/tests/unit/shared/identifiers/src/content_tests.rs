use std::collections::HashSet;
use systemprompt_identifiers::{SkillId, SourceId, CategoryId, ContentId, TagId, FileId, DbValue, ToDbValue};

#[test]
fn skill_id_generate_uuid_format() {
    let id = SkillId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn skill_id_generate_unique() {
    let ids: HashSet<String> = (0..10).map(|_| SkillId::generate().as_str().to_string()).collect();
    assert_eq!(ids.len(), 10);
}

#[test]
fn skill_id_serde_transparent() {
    let id = SkillId::new("skill-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"skill-1\"");
    let deserialized: SkillId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn source_id_serde_transparent() {
    let id = SourceId::new("source-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"source-1\"");
}

#[test]
fn category_id_serde_transparent() {
    let id = CategoryId::new("cat-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"cat-1\"");
}

#[test]
fn content_id_serde_transparent() {
    let id = ContentId::new("content-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"content-1\"");
}

#[test]
fn tag_id_serde_transparent() {
    let id = TagId::new("tag-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"tag-1\"");
}

#[test]
fn file_id_generate_uuid_format() {
    let id = FileId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn file_id_generate_unique() {
    let id1 = FileId::generate();
    let id2 = FileId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn file_id_serde_transparent() {
    let id = FileId::new("file-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"file-1\"");
    let deserialized: FileId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn all_content_ids_from_str_and_string_equal() {
    let s1: SourceId = "x".into();
    let s2: SourceId = String::from("x").into();
    assert_eq!(s1, s2);

    let c1: CategoryId = "y".into();
    let c2: CategoryId = String::from("y").into();
    assert_eq!(c1, c2);

    let co1: ContentId = "z".into();
    let co2: ContentId = String::from("z").into();
    assert_eq!(co1, co2);

    let t1: TagId = "w".into();
    let t2: TagId = String::from("w").into();
    assert_eq!(t1, t2);
}

#[test]
fn all_content_ids_to_db_value() {
    assert!(matches!(SkillId::new("a").to_db_value(), DbValue::String(ref s) if s == "a"));
    assert!(matches!(SourceId::new("b").to_db_value(), DbValue::String(ref s) if s == "b"));
    assert!(matches!(CategoryId::new("c").to_db_value(), DbValue::String(ref s) if s == "c"));
    assert!(matches!(ContentId::new("d").to_db_value(), DbValue::String(ref s) if s == "d"));
    assert!(matches!(TagId::new("e").to_db_value(), DbValue::String(ref s) if s == "e"));
    assert!(matches!(FileId::new("f").to_db_value(), DbValue::String(ref s) if s == "f"));
}

#[test]
fn all_content_ids_into_string() {
    let s: String = SkillId::new("sk").into();
    assert_eq!(s, "sk");
    let s: String = SourceId::new("so").into();
    assert_eq!(s, "so");
    let s: String = CategoryId::new("ca").into();
    assert_eq!(s, "ca");
    let s: String = ContentId::new("co").into();
    assert_eq!(s, "co");
    let s: String = TagId::new("ta").into();
    assert_eq!(s, "ta");
    let s: String = FileId::new("fi").into();
    assert_eq!(s, "fi");
}
