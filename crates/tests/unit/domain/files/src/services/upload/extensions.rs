//! Unit tests for FileValidator::get_extension

use systemprompt_files::FileValidator;

#[test]
fn test_file_validator_get_extension_from_filename() {
    let ext = FileValidator::get_extension("image/png", Some("myfile.jpeg"));
    assert_eq!(ext, "jpeg");
}

#[test]
fn test_file_validator_get_extension_no_filename() {
    let ext = FileValidator::get_extension("image/png", None);
    assert_eq!(ext, "png");
}

#[test]
fn test_file_validator_get_extension_invalid_filename() {
    let ext = FileValidator::get_extension("image/png", Some("noextension"));
    assert_eq!(ext, "png");
}

#[test]
fn test_file_validator_get_extension_empty_extension() {
    let ext = FileValidator::get_extension("image/jpeg", Some("file."));
    assert_eq!(ext, "jpg");
}

#[test]
fn test_file_validator_get_extension_too_long() {
    let ext = FileValidator::get_extension("image/png", Some("file.verylongextensionname"));
    assert_eq!(ext, "png");
}

#[test]
fn test_file_validator_get_extension_non_alphanumeric() {
    let ext = FileValidator::get_extension("image/png", Some("file.ex-t"));
    assert_eq!(ext, "png");
}

#[test]
fn test_file_validator_get_extension_image_types() {
    assert_eq!(FileValidator::get_extension("image/jpeg", None), "jpg");
    assert_eq!(FileValidator::get_extension("image/png", None), "png");
    assert_eq!(FileValidator::get_extension("image/gif", None), "gif");
    assert_eq!(FileValidator::get_extension("image/webp", None), "webp");
    assert_eq!(FileValidator::get_extension("image/svg+xml", None), "svg");
    assert_eq!(FileValidator::get_extension("image/bmp", None), "bmp");
    assert_eq!(FileValidator::get_extension("image/tiff", None), "tiff");
    assert_eq!(FileValidator::get_extension("image/x-icon", None), "ico");
    assert_eq!(
        FileValidator::get_extension("image/vnd.microsoft.icon", None),
        "ico"
    );
}

#[test]
fn test_file_validator_get_extension_document_types() {
    assert_eq!(FileValidator::get_extension("application/pdf", None), "pdf");
    assert_eq!(FileValidator::get_extension("text/plain", None), "txt");
    assert_eq!(FileValidator::get_extension("text/csv", None), "csv");
    assert_eq!(FileValidator::get_extension("text/markdown", None), "md");
    assert_eq!(FileValidator::get_extension("text/html", None), "html");
    assert_eq!(
        FileValidator::get_extension("application/json", None),
        "json"
    );
    assert_eq!(FileValidator::get_extension("application/xml", None), "xml");
    assert_eq!(FileValidator::get_extension("text/xml", None), "xml");
    assert_eq!(FileValidator::get_extension("application/rtf", None), "rtf");
    assert_eq!(
        FileValidator::get_extension("application/msword", None),
        "doc"
    );
    assert_eq!(
        FileValidator::get_extension(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            None
        ),
        "docx"
    );
    assert_eq!(
        FileValidator::get_extension("application/vnd.ms-excel", None),
        "xls"
    );
    assert_eq!(
        FileValidator::get_extension(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            None
        ),
        "xlsx"
    );
    assert_eq!(
        FileValidator::get_extension("application/vnd.ms-powerpoint", None),
        "ppt"
    );
    assert_eq!(
        FileValidator::get_extension(
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            None
        ),
        "pptx"
    );
}

#[test]
fn test_file_validator_get_extension_audio_types() {
    assert_eq!(FileValidator::get_extension("audio/mpeg", None), "mp3");
    assert_eq!(FileValidator::get_extension("audio/mp3", None), "mp3");
    assert_eq!(FileValidator::get_extension("audio/wav", None), "wav");
    assert_eq!(FileValidator::get_extension("audio/wave", None), "wav");
    assert_eq!(FileValidator::get_extension("audio/x-wav", None), "wav");
    assert_eq!(FileValidator::get_extension("audio/ogg", None), "ogg");
    assert_eq!(FileValidator::get_extension("audio/webm", None), "weba");
    assert_eq!(FileValidator::get_extension("audio/aac", None), "aac");
    assert_eq!(FileValidator::get_extension("audio/flac", None), "flac");
    assert_eq!(FileValidator::get_extension("audio/mp4", None), "m4a");
    assert_eq!(FileValidator::get_extension("audio/x-m4a", None), "m4a");
}

#[test]
fn test_file_validator_get_extension_video_types() {
    assert_eq!(FileValidator::get_extension("video/mp4", None), "mp4");
    assert_eq!(FileValidator::get_extension("video/webm", None), "webm");
    assert_eq!(FileValidator::get_extension("video/ogg", None), "ogv");
    assert_eq!(FileValidator::get_extension("video/quicktime", None), "mov");
    assert_eq!(FileValidator::get_extension("video/x-msvideo", None), "avi");
    assert_eq!(
        FileValidator::get_extension("video/x-matroska", None),
        "mkv"
    );
}

#[test]
fn test_file_validator_get_extension_unknown_type() {
    assert_eq!(
        FileValidator::get_extension("application/octet-stream", None),
        "bin"
    );
}
