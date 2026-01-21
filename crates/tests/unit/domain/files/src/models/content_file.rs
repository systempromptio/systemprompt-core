//! Unit tests for ContentFile and FileRole models

use chrono::Utc;
use systemprompt_files::{ContentFile, FileRole};
use systemprompt_identifiers::ContentId;

// ============================================================================
// FileRole::as_str Tests
// ============================================================================

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

// ============================================================================
// FileRole::parse Tests
// ============================================================================

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
    let result = FileRole::parse("unknown_role");
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Invalid file role"));
    assert!(err_msg.contains("unknown_role"));
}

#[test]
fn test_file_role_parse_empty_string() {
    let result = FileRole::parse("");
    assert!(result.is_err());
}

// ============================================================================
// FileRole Display Tests
// ============================================================================

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

// ============================================================================
// FileRole Default Tests
// ============================================================================

#[test]
fn test_file_role_default_is_attachment() {
    let default = FileRole::default();
    assert!(matches!(default, FileRole::Attachment));
}

// ============================================================================
// FileRole Serialization Tests
// ============================================================================

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
    // serde uses rename_all = "lowercase" which removes underscores
    let json = serde_json::to_string(&FileRole::OgImage).unwrap();
    assert_eq!(json, "\"ogimage\"");
}

#[test]
fn test_file_role_serialize_thumbnail() {
    let json = serde_json::to_string(&FileRole::Thumbnail).unwrap();
    assert_eq!(json, "\"thumbnail\"");
}

// ============================================================================
// FileRole Deserialization Tests
// ============================================================================

#[test]
fn test_file_role_deserialize_featured() {
    let role: FileRole = serde_json::from_str("\"featured\"").unwrap();
    assert!(matches!(role, FileRole::Featured));
}

#[test]
fn test_file_role_deserialize_attachment() {
    let role: FileRole = serde_json::from_str("\"attachment\"").unwrap();
    assert!(matches!(role, FileRole::Attachment));
}

#[test]
fn test_file_role_deserialize_inline() {
    let role: FileRole = serde_json::from_str("\"inline\"").unwrap();
    assert!(matches!(role, FileRole::Inline));
}

#[test]
fn test_file_role_deserialize_og_image() {
    // serde uses rename_all = "lowercase" which removes underscores
    let role: FileRole = serde_json::from_str("\"ogimage\"").unwrap();
    assert!(matches!(role, FileRole::OgImage));
}

#[test]
fn test_file_role_deserialize_thumbnail() {
    let role: FileRole = serde_json::from_str("\"thumbnail\"").unwrap();
    assert!(matches!(role, FileRole::Thumbnail));
}

#[test]
fn test_file_role_deserialize_invalid() {
    let result: Result<FileRole, _> = serde_json::from_str("\"invalid\"");
    assert!(result.is_err());
}

// ============================================================================
// FileRole Equality Tests
// ============================================================================

#[test]
fn test_file_role_equality() {
    assert_eq!(FileRole::Featured, FileRole::Featured);
    assert_eq!(FileRole::Attachment, FileRole::Attachment);
    assert_eq!(FileRole::Inline, FileRole::Inline);
    assert_eq!(FileRole::OgImage, FileRole::OgImage);
    assert_eq!(FileRole::Thumbnail, FileRole::Thumbnail);
}

#[test]
fn test_file_role_inequality() {
    assert_ne!(FileRole::Featured, FileRole::Attachment);
    assert_ne!(FileRole::Attachment, FileRole::Inline);
    assert_ne!(FileRole::Inline, FileRole::OgImage);
    assert_ne!(FileRole::OgImage, FileRole::Thumbnail);
    assert_ne!(FileRole::Thumbnail, FileRole::Featured);
}

// ============================================================================
// FileRole Copy/Clone Tests
// ============================================================================

#[test]
fn test_file_role_copy() {
    let role = FileRole::Featured;
    let copied = role;
    assert_eq!(role, copied);
}

#[test]
fn test_file_role_clone() {
    let role = FileRole::Inline;
    let cloned = role.clone();
    assert_eq!(role, cloned);
}

// ============================================================================
// FileRole Roundtrip Tests
// ============================================================================

#[test]
fn test_file_role_roundtrip_all_variants() {
    let roles = [
        FileRole::Featured,
        FileRole::Attachment,
        FileRole::Inline,
        FileRole::OgImage,
        FileRole::Thumbnail,
    ];

    for role in roles {
        let serialized = serde_json::to_string(&role).unwrap();
        let deserialized: FileRole = serde_json::from_str(&serialized).unwrap();
        assert_eq!(role, deserialized);
    }
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

// ============================================================================
// ContentFile Tests
// ============================================================================

fn create_test_content_file(role: &str) -> ContentFile {
    ContentFile {
        id: 1,
        content_id: ContentId::new("content_123"),
        file_id: uuid::Uuid::new_v4(),
        role: role.to_string(),
        display_order: 0,
        created_at: Utc::now(),
    }
}

#[test]
fn test_content_file_parsed_role_featured() {
    let file = create_test_content_file("featured");
    let role = file.parsed_role().unwrap();
    assert!(matches!(role, FileRole::Featured));
}

#[test]
fn test_content_file_parsed_role_attachment() {
    let file = create_test_content_file("attachment");
    let role = file.parsed_role().unwrap();
    assert!(matches!(role, FileRole::Attachment));
}

#[test]
fn test_content_file_parsed_role_inline() {
    let file = create_test_content_file("inline");
    let role = file.parsed_role().unwrap();
    assert!(matches!(role, FileRole::Inline));
}

#[test]
fn test_content_file_parsed_role_og_image() {
    let file = create_test_content_file("og_image");
    let role = file.parsed_role().unwrap();
    assert!(matches!(role, FileRole::OgImage));
}

#[test]
fn test_content_file_parsed_role_thumbnail() {
    let file = create_test_content_file("thumbnail");
    let role = file.parsed_role().unwrap();
    assert!(matches!(role, FileRole::Thumbnail));
}

#[test]
fn test_content_file_parsed_role_invalid() {
    let file = create_test_content_file("invalid_role");
    let result = file.parsed_role();
    assert!(result.is_err());
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
        role: "attachment".to_string(),
        display_order: 5,
        created_at: now,
    };

    assert_eq!(file.id, 42);
    assert_eq!(file.content_id.as_str(), "content_test");
    assert_eq!(file.file_id, file_id);
    assert_eq!(file.role, "attachment");
    assert_eq!(file.display_order, 5);
}

#[test]
fn test_content_file_clone() {
    let file = create_test_content_file("featured");
    let cloned = file.clone();

    assert_eq!(file.id, cloned.id);
    assert_eq!(file.role, cloned.role);
    assert_eq!(file.display_order, cloned.display_order);
}

#[test]
fn test_content_file_debug() {
    let file = create_test_content_file("attachment");
    let debug_str = format!("{:?}", file);

    assert!(debug_str.contains("ContentFile"));
    assert!(debug_str.contains("attachment"));
}

#[test]
fn test_content_file_serialization() {
    let file = create_test_content_file("inline");
    let json = serde_json::to_string(&file).unwrap();

    assert!(json.contains("\"role\":\"inline\""));
    assert!(json.contains("\"display_order\":0"));
}

#[test]
fn test_content_file_deserialization() {
    let now = Utc::now();
    let file_id = uuid::Uuid::new_v4();

    let json = format!(
        r#"{{"id":1,"content_id":"content_123","file_id":"{}","role":"thumbnail","display_order":3,"created_at":"{}"}}"#,
        file_id,
        now.to_rfc3339()
    );

    let file: ContentFile = serde_json::from_str(&json).unwrap();
    assert_eq!(file.id, 1);
    assert_eq!(file.role, "thumbnail");
    assert_eq!(file.display_order, 3);
}
