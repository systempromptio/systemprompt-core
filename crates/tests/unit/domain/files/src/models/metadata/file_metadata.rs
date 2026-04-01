//! Unit tests for FileMetadata, TypeSpecificMetadata serialization, and FileChecksums

use systemprompt_files::{
    FileChecksums, FileMetadata, ImageMetadata, TypeSpecificMetadata,
    AudioMetadata, DocumentMetadata, VideoMetadata,
};

#[test]
fn test_file_metadata_new() {
    let metadata = FileMetadata::new();
    assert!(metadata.checksums.is_none());
    assert!(metadata.type_specific.is_none());
}

#[test]
fn test_file_metadata_default() {
    let metadata = FileMetadata::default();
    assert!(metadata.checksums.is_none());
    assert!(metadata.type_specific.is_none());
}

#[test]
fn test_file_metadata_with_image() {
    let image = ImageMetadata::new().with_dimensions(800, 600);
    let metadata = FileMetadata::new().with_image(image);

    match metadata.type_specific.as_ref().expect("type_specific should be set") {
        TypeSpecificMetadata::Image(img) => {
            assert_eq!(img.width, Some(800));
            assert_eq!(img.height, Some(600));
        }
        _ => panic!("Expected Image metadata"),
    }
}

#[test]
fn test_file_metadata_with_document() {
    let doc = DocumentMetadata::new()
        .with_title("Test Document")
        .with_author("Test Author")
        .with_page_count(10);

    let metadata = FileMetadata::new().with_document(doc);

    match metadata.type_specific.as_ref().expect("type_specific should be set") {
        TypeSpecificMetadata::Document(d) => {
            assert_eq!(d.title, Some("Test Document".to_string()));
            assert_eq!(d.author, Some("Test Author".to_string()));
            assert_eq!(d.page_count, Some(10));
        }
        _ => panic!("Expected Document metadata"),
    }
}

#[test]
fn test_file_metadata_with_audio() {
    let audio = AudioMetadata::new()
        .with_duration_seconds(180.5)
        .with_sample_rate(44100)
        .with_channels(2);

    let metadata = FileMetadata::new().with_audio(audio);

    match metadata.type_specific.as_ref().expect("type_specific should be set") {
        TypeSpecificMetadata::Audio(a) => {
            assert_eq!(a.duration_seconds, Some(180.5));
            assert_eq!(a.sample_rate, Some(44100));
            assert_eq!(a.channels, Some(2));
        }
        _ => panic!("Expected Audio metadata"),
    }
}

#[test]
fn test_file_metadata_with_video() {
    let video = VideoMetadata::new()
        .with_dimensions(1920, 1080)
        .with_duration_seconds(3600.0)
        .with_frame_rate(30.0);

    let metadata = FileMetadata::new().with_video(video);

    match metadata.type_specific.as_ref().expect("type_specific should be set") {
        TypeSpecificMetadata::Video(v) => {
            assert_eq!(v.width, Some(1920));
            assert_eq!(v.height, Some(1080));
            assert_eq!(v.duration_seconds, Some(3600.0));
            assert_eq!(v.frame_rate, Some(30.0));
        }
        _ => panic!("Expected Video metadata"),
    }
}

#[test]
fn test_file_metadata_with_checksums() {
    let checksums = FileChecksums::new()
        .with_md5("abc123")
        .with_sha256("def456");

    let metadata = FileMetadata::new().with_checksums(checksums);

    let cs = metadata.checksums.expect("checksums should be set");
    assert_eq!(cs.md5, Some("abc123".to_string()));
    assert_eq!(cs.sha256, Some("def456".to_string()));
}

#[test]
fn test_file_metadata_builder_chain() {
    let checksums = FileChecksums::new().with_md5("test_md5");
    let image = ImageMetadata::new().with_dimensions(100, 100);

    let metadata = FileMetadata::new()
        .with_checksums(checksums)
        .with_image(image);

    metadata.checksums.as_ref().expect("checksums should be set");
    metadata.type_specific.as_ref().expect("type_specific should be set");
}

#[test]
fn test_file_metadata_serialize_empty() {
    let metadata = FileMetadata::new();
    let json = serde_json::to_string(&metadata).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_file_metadata_serialize_with_checksums() {
    let checksums = FileChecksums::new().with_md5("test");
    let metadata = FileMetadata::new().with_checksums(checksums);

    let json = serde_json::to_string(&metadata).unwrap();
    assert!(json.contains("checksums"));
    assert!(json.contains("md5"));
    assert!(json.contains("test"));
}

#[test]
fn test_file_metadata_deserialize_empty() {
    let metadata: FileMetadata = serde_json::from_str("{}").unwrap();
    assert!(metadata.checksums.is_none());
    assert!(metadata.type_specific.is_none());
}

#[test]
fn test_file_metadata_roundtrip() {
    let checksums = FileChecksums::new().with_sha256("sha_test");
    let image = ImageMetadata::new()
        .with_dimensions(640, 480)
        .with_alt_text("Test alt");
    let metadata = FileMetadata::new()
        .with_checksums(checksums)
        .with_image(image);

    let json = serde_json::to_string(&metadata).unwrap();
    let deserialized: FileMetadata = serde_json::from_str(&json).unwrap();

    deserialized.checksums.as_ref().expect("checksums should survive roundtrip");
    deserialized.type_specific.as_ref().expect("type_specific should survive roundtrip");
}

#[test]
fn test_type_specific_metadata_image_tag() {
    let image = ImageMetadata::new();
    let type_specific = TypeSpecificMetadata::Image(image);

    let json = serde_json::to_string(&type_specific).unwrap();
    assert!(json.contains("\"type\":\"image\""));
}

#[test]
fn test_type_specific_metadata_document_tag() {
    let doc = DocumentMetadata::new();
    let type_specific = TypeSpecificMetadata::Document(doc);

    let json = serde_json::to_string(&type_specific).unwrap();
    assert!(json.contains("\"type\":\"document\""));
}

#[test]
fn test_type_specific_metadata_audio_tag() {
    let audio = AudioMetadata::new();
    let type_specific = TypeSpecificMetadata::Audio(audio);

    let json = serde_json::to_string(&type_specific).unwrap();
    assert!(json.contains("\"type\":\"audio\""));
}

#[test]
fn test_type_specific_metadata_video_tag() {
    let video = VideoMetadata::new();
    let type_specific = TypeSpecificMetadata::Video(video);

    let json = serde_json::to_string(&type_specific).unwrap();
    assert!(json.contains("\"type\":\"video\""));
}
