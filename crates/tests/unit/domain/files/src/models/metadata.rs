//! Unit tests for FileMetadata, TypeSpecificMetadata, and related types

use systemprompt_core_files::{
    AudioMetadata, DocumentMetadata, FileChecksums, FileMetadata, ImageMetadata,
    TypeSpecificMetadata, VideoMetadata,
};

// ============================================================================
// FileMetadata Tests
// ============================================================================

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

    assert!(metadata.type_specific.is_some());
    match &metadata.type_specific {
        Some(TypeSpecificMetadata::Image(img)) => {
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

    assert!(metadata.type_specific.is_some());
    match &metadata.type_specific {
        Some(TypeSpecificMetadata::Document(d)) => {
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

    assert!(metadata.type_specific.is_some());
    match &metadata.type_specific {
        Some(TypeSpecificMetadata::Audio(a)) => {
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

    assert!(metadata.type_specific.is_some());
    match &metadata.type_specific {
        Some(TypeSpecificMetadata::Video(v)) => {
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

    assert!(metadata.checksums.is_some());
    let cs = metadata.checksums.unwrap();
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

    assert!(metadata.checksums.is_some());
    assert!(metadata.type_specific.is_some());
}

// ============================================================================
// FileMetadata Serialization Tests
// ============================================================================

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

    assert!(deserialized.checksums.is_some());
    assert!(deserialized.type_specific.is_some());
}

// ============================================================================
// TypeSpecificMetadata Serialization Tests
// ============================================================================

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

// ============================================================================
// DocumentMetadata Tests
// ============================================================================

#[test]
fn test_document_metadata_new() {
    let doc = DocumentMetadata::new();
    assert!(doc.title.is_none());
    assert!(doc.author.is_none());
    assert!(doc.page_count.is_none());
}

#[test]
fn test_document_metadata_default() {
    let doc = DocumentMetadata::default();
    assert!(doc.title.is_none());
    assert!(doc.author.is_none());
    assert!(doc.page_count.is_none());
}

#[test]
fn test_document_metadata_with_title() {
    let doc = DocumentMetadata::new().with_title("My Document");
    assert_eq!(doc.title, Some("My Document".to_string()));
}

#[test]
fn test_document_metadata_with_author() {
    let doc = DocumentMetadata::new().with_author("John Doe");
    assert_eq!(doc.author, Some("John Doe".to_string()));
}

#[test]
fn test_document_metadata_with_page_count() {
    let doc = DocumentMetadata::new().with_page_count(42);
    assert_eq!(doc.page_count, Some(42));
}

#[test]
fn test_document_metadata_builder_chain() {
    let doc = DocumentMetadata::new()
        .with_title("Title")
        .with_author("Author")
        .with_page_count(100);

    assert_eq!(doc.title, Some("Title".to_string()));
    assert_eq!(doc.author, Some("Author".to_string()));
    assert_eq!(doc.page_count, Some(100));
}

#[test]
fn test_document_metadata_clone() {
    let doc = DocumentMetadata::new().with_title("Test");
    let cloned = doc.clone();
    assert_eq!(doc.title, cloned.title);
}

// ============================================================================
// AudioMetadata Tests
// ============================================================================

#[test]
fn test_audio_metadata_new() {
    let audio = AudioMetadata::new();
    assert!(audio.duration_seconds.is_none());
    assert!(audio.sample_rate.is_none());
    assert!(audio.channels.is_none());
}

#[test]
fn test_audio_metadata_default() {
    let audio = AudioMetadata::default();
    assert!(audio.duration_seconds.is_none());
    assert!(audio.sample_rate.is_none());
    assert!(audio.channels.is_none());
}

#[test]
fn test_audio_metadata_with_duration() {
    let audio = AudioMetadata::new().with_duration_seconds(120.5);
    assert_eq!(audio.duration_seconds, Some(120.5));
}

#[test]
fn test_audio_metadata_with_sample_rate() {
    let audio = AudioMetadata::new().with_sample_rate(48000);
    assert_eq!(audio.sample_rate, Some(48000));
}

#[test]
fn test_audio_metadata_with_channels() {
    let audio = AudioMetadata::new().with_channels(2);
    assert_eq!(audio.channels, Some(2));
}

#[test]
fn test_audio_metadata_builder_chain() {
    let audio = AudioMetadata::new()
        .with_duration_seconds(300.0)
        .with_sample_rate(44100)
        .with_channels(2);

    assert_eq!(audio.duration_seconds, Some(300.0));
    assert_eq!(audio.sample_rate, Some(44100));
    assert_eq!(audio.channels, Some(2));
}

#[test]
fn test_audio_metadata_copy() {
    let audio = AudioMetadata::new().with_channels(1);
    let copied = audio;
    assert_eq!(audio.channels, copied.channels);
}

// ============================================================================
// VideoMetadata Tests
// ============================================================================

#[test]
fn test_video_metadata_new() {
    let video = VideoMetadata::new();
    assert!(video.width.is_none());
    assert!(video.height.is_none());
    assert!(video.duration_seconds.is_none());
    assert!(video.frame_rate.is_none());
}

#[test]
fn test_video_metadata_default() {
    let video = VideoMetadata::default();
    assert!(video.width.is_none());
    assert!(video.height.is_none());
    assert!(video.duration_seconds.is_none());
    assert!(video.frame_rate.is_none());
}

#[test]
fn test_video_metadata_with_dimensions() {
    let video = VideoMetadata::new().with_dimensions(1280, 720);
    assert_eq!(video.width, Some(1280));
    assert_eq!(video.height, Some(720));
}

#[test]
fn test_video_metadata_with_duration() {
    let video = VideoMetadata::new().with_duration_seconds(7200.0);
    assert_eq!(video.duration_seconds, Some(7200.0));
}

#[test]
fn test_video_metadata_with_frame_rate() {
    let video = VideoMetadata::new().with_frame_rate(60.0);
    assert_eq!(video.frame_rate, Some(60.0));
}

#[test]
fn test_video_metadata_builder_chain() {
    let video = VideoMetadata::new()
        .with_dimensions(3840, 2160)
        .with_duration_seconds(5400.0)
        .with_frame_rate(24.0);

    assert_eq!(video.width, Some(3840));
    assert_eq!(video.height, Some(2160));
    assert_eq!(video.duration_seconds, Some(5400.0));
    assert_eq!(video.frame_rate, Some(24.0));
}

#[test]
fn test_video_metadata_copy() {
    let video = VideoMetadata::new().with_frame_rate(30.0);
    let copied = video;
    assert_eq!(video.frame_rate, copied.frame_rate);
}

// ============================================================================
// FileChecksums Tests
// ============================================================================

#[test]
fn test_file_checksums_new() {
    let checksums = FileChecksums::new();
    assert!(checksums.md5.is_none());
    assert!(checksums.sha256.is_none());
}

#[test]
fn test_file_checksums_default() {
    let checksums = FileChecksums::default();
    assert!(checksums.md5.is_none());
    assert!(checksums.sha256.is_none());
}

#[test]
fn test_file_checksums_with_md5() {
    let checksums = FileChecksums::new().with_md5("d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(
        checksums.md5,
        Some("d41d8cd98f00b204e9800998ecf8427e".to_string())
    );
}

#[test]
fn test_file_checksums_with_sha256() {
    let checksums = FileChecksums::new()
        .with_sha256("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(
        checksums.sha256,
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
    );
}

#[test]
fn test_file_checksums_builder_chain() {
    let checksums = FileChecksums::new()
        .with_md5("md5_hash")
        .with_sha256("sha256_hash");

    assert_eq!(checksums.md5, Some("md5_hash".to_string()));
    assert_eq!(checksums.sha256, Some("sha256_hash".to_string()));
}

#[test]
fn test_file_checksums_clone() {
    let checksums = FileChecksums::new().with_md5("test");
    let cloned = checksums.clone();
    assert_eq!(checksums.md5, cloned.md5);
}

#[test]
fn test_file_checksums_serialize_empty() {
    let checksums = FileChecksums::new();
    let json = serde_json::to_string(&checksums).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn test_file_checksums_serialize_with_values() {
    let checksums = FileChecksums::new()
        .with_md5("abc")
        .with_sha256("def");

    let json = serde_json::to_string(&checksums).unwrap();
    assert!(json.contains("md5"));
    assert!(json.contains("abc"));
    assert!(json.contains("sha256"));
    assert!(json.contains("def"));
}

#[test]
fn test_file_checksums_roundtrip() {
    let checksums = FileChecksums::new()
        .with_md5("md5_value")
        .with_sha256("sha256_value");

    let json = serde_json::to_string(&checksums).unwrap();
    let deserialized: FileChecksums = serde_json::from_str(&json).unwrap();

    assert_eq!(checksums.md5, deserialized.md5);
    assert_eq!(checksums.sha256, deserialized.sha256);
}
