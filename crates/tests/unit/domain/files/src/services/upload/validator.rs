//! Unit tests for FileValidator (MIME type validation, categorization)

use systemprompt_files::{
    AllowedFileTypes, FileCategory, FileUploadConfig, FileValidationError, FileValidator,
};

fn default_config() -> FileUploadConfig {
    FileUploadConfig::default()
}

fn disabled_config() -> FileUploadConfig {
    FileUploadConfig {
        enabled: false,
        ..Default::default()
    }
}

fn small_size_config() -> FileUploadConfig {
    FileUploadConfig {
        max_file_size_bytes: 1024,
        ..Default::default()
    }
}

fn images_only_config() -> FileUploadConfig {
    FileUploadConfig {
        allowed_types: AllowedFileTypes {
            images: true,
            documents: false,
            audio: false,
            video: false,
        },
        ..Default::default()
    }
}

fn all_types_config() -> FileUploadConfig {
    FileUploadConfig {
        allowed_types: AllowedFileTypes {
            images: true,
            documents: true,
            audio: true,
            video: true,
        },
        ..Default::default()
    }
}

#[test]
fn test_file_validator_new() {
    let config = default_config();
    let validator = FileValidator::new(config);
    let debug_str = format!("{:?}", validator);
    assert!(debug_str.contains("FileValidator"));
}

#[test]
fn test_file_validator_validate_image_png() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/png", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_jpeg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/jpeg", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_gif() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/gif", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_webp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/webp", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_svg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/svg+xml", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_bmp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/bmp", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_tiff() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/tiff", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/x-icon", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_microsoft_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/vnd.microsoft.icon", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_document_pdf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/pdf", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_word() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/msword", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_docx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        1000,
    );
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_excel() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-excel", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_xlsx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        1000,
    );
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_powerpoint() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-powerpoint", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_pptx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        1000,
    );
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_text() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/plain", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_csv() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/csv", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_markdown() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/markdown", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_html() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/html", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_json() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/json", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/xml", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_text_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/xml", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_rtf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/rtf", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_audio_mpeg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mpeg", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_mp3() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mp3", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_wav() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/wav", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_wave() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/wave", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_x_wav() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/x-wav", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_ogg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/ogg", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_webm() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/webm", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_aac() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/aac", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_flac() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/flac", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_mp4() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mp4", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_m4a() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/x-m4a", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_video_mp4() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/mp4", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_webm() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/webm", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_ogg() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/ogg", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_quicktime() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/quicktime", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_avi() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/x-msvideo", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_matroska() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/x-matroska", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Video);
}

#[test]
fn test_file_validator_uploads_disabled() {
    let validator = FileValidator::new(disabled_config());
    let result = validator.validate("image/png", 1000);
    match result.unwrap_err() {
        FileValidationError::UploadsDisabled => {}
        _ => panic!("Expected UploadsDisabled error"),
    }
}

#[test]
fn test_file_validator_file_too_large() {
    let validator = FileValidator::new(small_size_config());
    let result = validator.validate("image/png", 2000);
    match result.unwrap_err() {
        FileValidationError::FileTooLarge { size, max } => {
            assert_eq!(size, 2000);
            assert_eq!(max, 1024);
        }
        _ => panic!("Expected FileTooLarge error"),
    }
}

#[test]
fn test_file_validator_type_not_allowed() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/octet-stream", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeNotAllowed { mime_type } => {
            assert_eq!(mime_type, "application/octet-stream");
        }
        _ => panic!("Expected TypeNotAllowed error"),
    }
}

#[test]
fn test_file_validator_blocked_executable() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-executable", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { mime_type } => {
            assert_eq!(mime_type, "application/x-executable");
        }
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_shell_script() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-sh", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_javascript() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/javascript", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_php() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-httpd-php", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_java_class() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-java-class", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_category_disabled_video() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("video/mp4", 1000);
    match result.unwrap_err() {
        FileValidationError::CategoryDisabled { category } => {
            assert_eq!(category, "video");
        }
        _ => panic!("Expected CategoryDisabled error"),
    }
}

#[test]
fn test_file_validator_category_disabled_documents() {
    let validator = FileValidator::new(images_only_config());
    let result = validator.validate("application/pdf", 1000);
    match result.unwrap_err() {
        FileValidationError::CategoryDisabled { category } => {
            assert_eq!(category, "document");
        }
        _ => panic!("Expected CategoryDisabled error"),
    }
}

#[test]
fn test_file_validator_category_disabled_audio() {
    let validator = FileValidator::new(images_only_config());
    let result = validator.validate("audio/mpeg", 1000);
    match result.unwrap_err() {
        FileValidationError::CategoryDisabled { category } => {
            assert_eq!(category, "audio");
        }
        _ => panic!("Expected CategoryDisabled error"),
    }
}

#[test]
fn test_file_validator_case_insensitive() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("IMAGE/PNG", 1000);
    assert_eq!(result.expect("validation should succeed"), FileCategory::Image);
}
