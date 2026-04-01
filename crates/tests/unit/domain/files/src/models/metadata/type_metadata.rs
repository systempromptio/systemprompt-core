//! Unit tests for DocumentMetadata, AudioMetadata, VideoMetadata, and FileChecksums

use systemprompt_files::{AudioMetadata, DocumentMetadata, FileChecksums, VideoMetadata};

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
