pub const SUPPORTED_IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];

pub const SUPPORTED_AUDIO_TYPES: &[&str] = &[
    "audio/wav",
    "audio/mp3",
    "audio/mpeg",
    "audio/aiff",
    "audio/aac",
    "audio/ogg",
    "audio/flac",
];

pub const SUPPORTED_VIDEO_TYPES: &[&str] = &[
    "video/mp4",
    "video/mpeg",
    "video/mov",
    "video/avi",
    "video/x-flv",
    "video/mpg",
    "video/webm",
    "video/wmv",
    "video/3gpp",
];

pub const SUPPORTED_TEXT_TYPES: &[&str] = &[
    "text/plain",
    "text/markdown",
    "text/csv",
    "text/html",
    "text/xml",
    "application/json",
    "application/xml",
];

pub fn is_supported_image(mime_type: &str) -> bool {
    SUPPORTED_IMAGE_TYPES
        .iter()
        .any(|&t| mime_type.starts_with(t))
}

pub fn is_supported_audio(mime_type: &str) -> bool {
    SUPPORTED_AUDIO_TYPES
        .iter()
        .any(|&t| mime_type.starts_with(t))
}

pub fn is_supported_video(mime_type: &str) -> bool {
    SUPPORTED_VIDEO_TYPES
        .iter()
        .any(|&t| mime_type.starts_with(t))
}

pub fn is_supported_text(mime_type: &str) -> bool {
    SUPPORTED_TEXT_TYPES
        .iter()
        .any(|&t| mime_type.starts_with(t))
}

pub fn is_supported_media(mime_type: &str) -> bool {
    is_supported_image(mime_type)
        || is_supported_audio(mime_type)
        || is_supported_video(mime_type)
        || is_supported_text(mime_type)
}
