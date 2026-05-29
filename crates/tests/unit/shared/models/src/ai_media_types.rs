use systemprompt_models::ai::{
    SUPPORTED_AUDIO_TYPES, SUPPORTED_IMAGE_TYPES, SUPPORTED_TEXT_TYPES, SUPPORTED_VIDEO_TYPES,
    is_supported_audio, is_supported_image, is_supported_media, is_supported_text,
    is_supported_video,
};

#[test]
fn image_types_includes_jpeg_png_gif_webp() {
    assert!(SUPPORTED_IMAGE_TYPES.contains(&"image/jpeg"));
    assert!(SUPPORTED_IMAGE_TYPES.contains(&"image/png"));
    assert!(SUPPORTED_IMAGE_TYPES.contains(&"image/gif"));
    assert!(SUPPORTED_IMAGE_TYPES.contains(&"image/webp"));
}

#[test]
fn is_supported_image_true_for_known_types() {
    assert!(is_supported_image("image/jpeg"));
    assert!(is_supported_image("image/png"));
    assert!(is_supported_image("image/gif"));
    assert!(is_supported_image("image/webp"));
}

#[test]
fn is_supported_image_false_for_unknown() {
    assert!(!is_supported_image("image/svg+xml"));
    assert!(!is_supported_image("video/mp4"));
    assert!(!is_supported_image("text/plain"));
}

#[test]
fn audio_types_includes_wav_and_mp3() {
    assert!(SUPPORTED_AUDIO_TYPES.contains(&"audio/wav"));
    assert!(SUPPORTED_AUDIO_TYPES.contains(&"audio/mp3"));
}

#[test]
fn is_supported_audio_true_for_known_types() {
    assert!(is_supported_audio("audio/wav"));
    assert!(is_supported_audio("audio/mp3"));
    assert!(is_supported_audio("audio/mpeg"));
    assert!(is_supported_audio("audio/aiff"));
    assert!(is_supported_audio("audio/ogg"));
    assert!(is_supported_audio("audio/flac"));
}

#[test]
fn is_supported_audio_false_for_unknown() {
    assert!(!is_supported_audio("audio/x-wav2"));
    assert!(!is_supported_audio("image/jpeg"));
}

#[test]
fn video_types_includes_mp4_mpeg() {
    assert!(SUPPORTED_VIDEO_TYPES.contains(&"video/mp4"));
    assert!(SUPPORTED_VIDEO_TYPES.contains(&"video/mpeg"));
}

#[test]
fn is_supported_video_true_for_known_types() {
    assert!(is_supported_video("video/mp4"));
    assert!(is_supported_video("video/mpeg"));
    assert!(is_supported_video("video/webm"));
}

#[test]
fn is_supported_video_false_for_audio() {
    assert!(!is_supported_video("audio/mp3"));
}

#[test]
fn text_types_includes_plain_markdown_json() {
    assert!(SUPPORTED_TEXT_TYPES.contains(&"text/plain"));
    assert!(SUPPORTED_TEXT_TYPES.contains(&"text/markdown"));
    assert!(SUPPORTED_TEXT_TYPES.contains(&"application/json"));
}

#[test]
fn is_supported_text_true_for_known_types() {
    assert!(is_supported_text("text/plain"));
    assert!(is_supported_text("text/markdown"));
    assert!(is_supported_text("text/csv"));
    assert!(is_supported_text("text/html"));
    assert!(is_supported_text("application/json"));
    assert!(is_supported_text("application/xml"));
}

#[test]
fn is_supported_text_false_for_binary() {
    assert!(!is_supported_text("image/png"));
    assert!(!is_supported_text("application/octet-stream"));
}

#[test]
fn is_supported_media_true_for_all_categories() {
    assert!(is_supported_media("image/png"));
    assert!(is_supported_media("audio/wav"));
    assert!(is_supported_media("video/mp4"));
    assert!(is_supported_media("text/plain"));
}

#[test]
fn is_supported_media_false_for_binary_blob() {
    assert!(!is_supported_media("application/octet-stream"));
    assert!(!is_supported_media(""));
}
