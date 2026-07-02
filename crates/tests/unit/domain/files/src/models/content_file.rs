//! Unit tests for ContentFile and FileRole models

use chrono::Utc;
use systemprompt_files::{ContentFile, FileRole};
use systemprompt_identifiers::ContentId;

#[test]
fn test_file_role_featured_as_str() {
    assert_eq!(FileRole::Featured.as_str(), "featured");
}

#[test]
fn test_file_role_attachment_as_str() {
    assert_eq!(FileRole::Attachment.as_str(), "attachment");
}

#[test]
fn test_file_role_inline_as_str() {
    assert_eq!(FileRole::Inline.as_str(), "inline");
}

#[test]
fn test_file_role_og_image_as_str() {
    assert_eq!(FileRole::OgImage.as_str(), "og_image");
}

#[test]
fn test_file_role_thumbnail_as_str() {
    assert_eq!(FileRole::Thumbnail.as_str(), "thumbnail");
}

#[test]
fn test_file_role_parse_featured() {
    let role = FileRole::parse("featured").unwrap();
    assert!(matches!(role, FileRole::Featured));
}

#[test]
fn test_file_role_parse_attachment() {
    let role = FileRole::parse("attachment").unwrap();
    assert!(matches!(role, FileRole::Attachment));
}

#[test]
fn test_file_role_parse_inline() {
    let role = FileRole::parse("inline").unwrap();
    assert!(matches!(role, FileRole::Inline));
}

#[test]
fn test_file_role_parse_og_image() {
    let role = FileRole::parse("og_image").unwrap();
    assert!(matches!(role, FileRole::OgImage));
}

#[test]
fn test_file_role_parse_thumbnail() {
    let role = FileRole::parse("thumbnail").unwrap();
    assert!(matches!(role, FileRole::Thumbnail));
}

#[test]
fn test_file_role_parse_case_insensitive() {
    let role = FileRole::parse("FEATURED").unwrap();
    assert!(matches!(role, FileRole::Featured));

    let role = FileRole::parse("Attachment").unwrap();
    assert!(matches!(role, FileRole::Attachment));

    let role = FileRole::parse("InLiNe").unwrap();
    assert!(matches!(role, FileRole::Inline));
}

#[test]
fn test_file_role_parse_invalid() {
    let err_msg = FileRole::parse("unknown_role").unwrap_err().to_string();
    assert!(err_msg.contains("invalid file role"));
    assert!(err_msg.contains("unknown_role"));
}

#[test]
fn test_file_role_parse_empty_string() {
    let result = FileRole::parse("");
    result.unwrap_err();
}

#[test]
fn test_file_role_display_featured() {
    let role = FileRole::Featured;
    assert_eq!(format!("{}", role), "featured");
}

#[test]
fn test_file_role_display_attachment() {
    let role = FileRole::Attachment;
    assert_eq!(format!("{}", role), "attachment");
}

#[test]
fn test_file_role_display_inline() {
    let role = FileRole::Inline;
    assert_eq!(format!("{}", role), "inline");
}

#[test]
fn test_file_role_display_og_image() {
    let role = FileRole::OgImage;
    assert_eq!(format!("{}", role), "og_image");
}

#[test]
fn test_file_role_display_thumbnail() {
    let role = FileRole::Thumbnail;
    assert_eq!(format!("{}", role), "thumbnail");
}

#[test]
fn test_file_role_default_is_attachment() {
    let default = FileRole::default();
    assert!(matches!(default, FileRole::Attachment));
}

#[test]
fn test_file_role_serialize_featured() {
    let json = serde_json::to_string(&FileRole::Featured).unwrap();
    assert_eq!(json, "\"featured\"");
}

#[test]
fn test_file_role_serialize_attachment() {
    let json = serde_json::to_string(&FileRole::Attachment).unwrap();
    assert_eq!(json, "\"attachment\"");
}

#[test]
fn test_file_role_serialize_inline() {
    let json = serde_json::to_string(&FileRole::Inline).unwrap();
    assert_eq!(json, "\"inline\"");
}

#[test]
fn test_file_role_serialize_og_image() {
    let json = serde_json::to_string(&FileRole::OgImage).unwrap();
    assert_eq!(json, "\"og_image\"");
}

#[test]
fn test_file_role_serialize_thumbnail() {
    let json = serde_json::to_string(&FileRole::Thumbnail).unwrap();
    assert_eq!(json, "\"thumbnail\"");
}

#[test]
fn test_file_role_as_str_parse_roundtrip() {
    let roles = [
        FileRole::Featured,
        FileRole::Attachment,
        FileRole::Inline,
        FileRole::OgImage,
        FileRole::Thumbnail,
    ];

    for role in roles {
        let str_repr = role.as_str();
        let parsed = FileRole::parse(str_repr).unwrap();
        assert_eq!(role, parsed);
    }
}

fn create_test_content_file(role: FileRole) -> ContentFile {
    ContentFile {
        id: 1,
        content_id: ContentId::new("content_123"),
        file_id: uuid::Uuid::new_v4(),
        role,
        display_order: 0,
        created_at: Utc::now(),
    }
}

#[test]
fn test_content_file_role_roundtrip() {
    let roles = [
        FileRole::Featured,
        FileRole::Attachment,
        FileRole::Inline,
        FileRole::OgImage,
        FileRole::Thumbnail,
    ];
    for role in roles {
        let file = create_test_content_file(role);
        assert_eq!(file.role, role);
        assert_eq!(FileRole::parse(file.role.as_str()).unwrap(), role);
    }
}

#[test]
fn test_content_file_struct_fields() {
    let now = Utc::now();
    let file_id = uuid::Uuid::new_v4();
    let content_id = ContentId::new("content_test");

    let file = ContentFile {
        id: 42,
        content_id: content_id.clone(),
        file_id,
        role: FileRole::Attachment,
        display_order: 5,
        created_at: now,
    };

    assert_eq!(file.id, 42);
    assert_eq!(file.content_id.as_str(), "content_test");
    assert_eq!(file.file_id, file_id);
    assert_eq!(file.role, FileRole::Attachment);
    assert_eq!(file.display_order, 5);
}

#[test]
fn test_content_file_clone() {
    let file = create_test_content_file(FileRole::Featured);
    let cloned = file.clone();

    assert_eq!(file.id, cloned.id);
    assert_eq!(file.role, cloned.role);
    assert_eq!(file.display_order, cloned.display_order);
}

#[test]
fn test_content_file_debug() {
    let file = create_test_content_file(FileRole::Attachment);
    let debug_str = format!("{:?}", file);

    assert!(debug_str.contains("ContentFile"));
    assert!(debug_str.contains("Attachment"));
}

#[test]
fn test_content_file_serialization() {
    let file = create_test_content_file(FileRole::Inline);
    let json = serde_json::to_string(&file).unwrap();

    assert!(json.contains("\"role\":\"inline\""));
    assert!(json.contains("\"display_order\":0"));
}
