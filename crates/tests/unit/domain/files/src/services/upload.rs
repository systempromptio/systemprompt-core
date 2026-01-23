//! Unit tests for file upload services
//!
//! Tests cover:
//! - FileValidator (MIME type validation, categorization, extension mapping)
//! - FileCategory (storage subdirs, display names)
//! - FileUploadRequest and builder
//! - UploadedFile struct
//! - Error types (FileValidationError, FileUploadError)

use systemprompt_files::{
    AllowedFileTypes, FileCategory, FileUploadConfig, FileUploadError, FileUploadRequest,
    FileUploadRequestBuilder, FileValidationError, FileValidator, UploadedFile,
};
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

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
fn test_file_category_storage_subdir_image() {
    assert_eq!(FileCategory::Image.storage_subdir(), "images");
}

#[test]
fn test_file_category_storage_subdir_document() {
    assert_eq!(FileCategory::Document.storage_subdir(), "documents");
}

#[test]
fn test_file_category_storage_subdir_audio() {
    assert_eq!(FileCategory::Audio.storage_subdir(), "audio");
}

#[test]
fn test_file_category_storage_subdir_video() {
    assert_eq!(FileCategory::Video.storage_subdir(), "video");
}

#[test]
fn test_file_category_display_name_image() {
    assert_eq!(FileCategory::Image.display_name(), "image");
}

#[test]
fn test_file_category_display_name_document() {
    assert_eq!(FileCategory::Document.display_name(), "document");
}

#[test]
fn test_file_category_display_name_audio() {
    assert_eq!(FileCategory::Audio.display_name(), "audio");
}

#[test]
fn test_file_category_display_name_video() {
    assert_eq!(FileCategory::Video.display_name(), "video");
}

#[test]
fn test_file_category_clone() {
    let category = FileCategory::Image;
    let cloned = category;
    assert_eq!(category, cloned);
}

#[test]
fn test_file_category_debug() {
    let debug_str = format!("{:?}", FileCategory::Document);
    assert!(debug_str.contains("Document"));
}

#[test]
fn test_file_category_equality() {
    assert_eq!(FileCategory::Image, FileCategory::Image);
    assert_ne!(FileCategory::Image, FileCategory::Document);
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
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_jpeg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/jpeg", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_gif() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/gif", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_webp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/webp", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_svg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/svg+xml", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_bmp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/bmp", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_tiff() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/tiff", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/x-icon", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_microsoft_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/vnd.microsoft.icon", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_document_pdf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/pdf", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_word() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/msword", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_docx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        1000,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_excel() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-excel", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_xlsx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        1000,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_powerpoint() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-powerpoint", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_pptx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        1000,
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_text() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/plain", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_csv() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/csv", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_markdown() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/markdown", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_html() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/html", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_json() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/json", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/xml", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_text_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/xml", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_document_rtf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/rtf", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Document);
}

#[test]
fn test_file_validator_validate_audio_mpeg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mpeg", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_mp3() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mp3", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_wav() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/wav", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_wave() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/wave", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_x_wav() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/x-wav", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_ogg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/ogg", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_webm() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/webm", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_aac() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/aac", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_flac() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/flac", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_mp4() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/mp4", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_audio_m4a() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("audio/x-m4a", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Audio);
}

#[test]
fn test_file_validator_validate_video_mp4() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/mp4", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_webm() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/webm", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_ogg() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/ogg", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_quicktime() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/quicktime", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_avi() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/x-msvideo", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_validate_video_matroska() {
    let validator = FileValidator::new(all_types_config());
    let result = validator.validate("video/x-matroska", 1000);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Video);
}

#[test]
fn test_file_validator_uploads_disabled() {
    let validator = FileValidator::new(disabled_config());
    let result = validator.validate("image/png", 1000);
    assert!(result.is_err());
    match result.unwrap_err() {
        FileValidationError::UploadsDisabled => {}
        _ => panic!("Expected UploadsDisabled error"),
    }
}

#[test]
fn test_file_validator_file_too_large() {
    let validator = FileValidator::new(small_size_config());
    let result = validator.validate("image/png", 2000);
    assert!(result.is_err());
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
    assert!(result.is_err());
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
    assert!(result.is_err());
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
    assert!(result.is_err());
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_javascript() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/javascript", 1000);
    assert!(result.is_err());
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_php() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-httpd-php", 1000);
    assert!(result.is_err());
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_java_class() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-java-class", 1000);
    assert!(result.is_err());
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {}
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_category_disabled_video() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("video/mp4", 1000);
    assert!(result.is_err());
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
    assert!(result.is_err());
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
    assert!(result.is_err());
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
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), FileCategory::Image);
}

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

#[test]
fn test_file_validation_error_display_uploads_disabled() {
    let err = FileValidationError::UploadsDisabled;
    assert_eq!(format!("{}", err), "File uploads are disabled");
}

#[test]
fn test_file_validation_error_display_file_too_large() {
    let err = FileValidationError::FileTooLarge {
        size: 1000,
        max: 500,
    };
    assert_eq!(
        format!("{}", err),
        "File size 1000 bytes exceeds maximum allowed 500 bytes"
    );
}

#[test]
fn test_file_validation_error_display_type_not_allowed() {
    let err = FileValidationError::TypeNotAllowed {
        mime_type: "application/octet-stream".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File type 'application/octet-stream' is not allowed"
    );
}

#[test]
fn test_file_validation_error_display_type_blocked() {
    let err = FileValidationError::TypeBlocked {
        mime_type: "application/x-executable".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File type 'application/x-executable' is blocked for security reasons"
    );
}

#[test]
fn test_file_validation_error_display_category_disabled() {
    let err = FileValidationError::CategoryDisabled {
        category: "video".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File category 'video' is disabled in configuration"
    );
}

#[test]
fn test_file_validation_error_debug() {
    let err = FileValidationError::UploadsDisabled;
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("UploadsDisabled"));
}

#[test]
fn test_file_upload_error_display_persistence_disabled() {
    let err = FileUploadError::PersistenceDisabled;
    assert_eq!(format!("{}", err), "File persistence is disabled");
}

#[test]
fn test_file_upload_error_display_validation() {
    let err = FileUploadError::Validation(FileValidationError::UploadsDisabled);
    assert!(format!("{}", err).contains("Validation failed"));
}

#[test]
fn test_file_upload_error_display_database() {
    let err = FileUploadError::Database("connection failed".to_string());
    assert_eq!(format!("{}", err), "Database error: connection failed");
}

#[test]
fn test_file_upload_error_display_config() {
    let err = FileUploadError::Config("missing path".to_string());
    assert_eq!(format!("{}", err), "Configuration error: missing path");
}

#[test]
fn test_file_upload_error_display_base64_too_large() {
    let err = FileUploadError::Base64TooLarge {
        encoded_size: 100_000_000,
    };
    assert!(format!("{}", err).contains("Base64 input too large"));
}

#[test]
fn test_file_upload_error_display_path_validation() {
    let err = FileUploadError::PathValidation("invalid characters".to_string());
    assert_eq!(
        format!("{}", err),
        "Path validation failed: invalid characters"
    );
}

#[test]
fn test_file_upload_error_from_validation_error() {
    let validation_err = FileValidationError::UploadsDisabled;
    let upload_err: FileUploadError = validation_err.into();
    match upload_err {
        FileUploadError::Validation(_) => {}
        _ => panic!("Expected Validation variant"),
    }
}

#[test]
fn test_file_upload_request_builder_new() {
    let context_id = ContextId::new("ctx_123");
    let builder =
        FileUploadRequestBuilder::new("image/png", "base64data==", context_id);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("FileUploadRequestBuilder"));
}

#[test]
fn test_file_upload_request_builder_build() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id).build();

    assert_eq!(request.mime_type, "image/png");
    assert_eq!(request.bytes_base64, "base64data==");
    assert_eq!(request.context_id.as_str(), "ctx_123");
    assert!(request.name.is_none());
    assert!(request.user_id.is_none());
    assert!(request.session_id.is_none());
    assert!(request.trace_id.is_none());
}

#[test]
fn test_file_upload_request_builder_with_name() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_name("myfile.png")
        .build();

    assert_eq!(request.name, Some("myfile.png".to_string()));
}

#[test]
fn test_file_upload_request_builder_with_user_id() {
    let context_id = ContextId::new("ctx_123");
    let user_id = UserId::new("user_abc");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_user_id(user_id)
        .build();

    assert!(request.user_id.is_some());
    assert_eq!(request.user_id.as_ref().unwrap().as_str(), "user_abc");
}

#[test]
fn test_file_upload_request_builder_with_session_id() {
    let context_id = ContextId::new("ctx_123");
    let session_id = SessionId::new("sess_xyz");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_session_id(session_id)
        .build();

    assert!(request.session_id.is_some());
    assert_eq!(request.session_id.as_ref().unwrap().as_str(), "sess_xyz");
}

#[test]
fn test_file_upload_request_builder_with_trace_id() {
    let context_id = ContextId::new("ctx_123");
    let trace_id = TraceId::new("trace_def");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_trace_id(trace_id)
        .build();

    assert!(request.trace_id.is_some());
    assert_eq!(request.trace_id.as_ref().unwrap().as_str(), "trace_def");
}

#[test]
fn test_file_upload_request_builder_full_chain() {
    let context_id = ContextId::new("ctx_123");
    let user_id = UserId::new("user_abc");
    let session_id = SessionId::new("sess_xyz");
    let trace_id = TraceId::new("trace_def");

    let request = FileUploadRequestBuilder::new("application/pdf", "pdfdata==", context_id)
        .with_name("document.pdf")
        .with_user_id(user_id)
        .with_session_id(session_id)
        .with_trace_id(trace_id)
        .build();

    assert_eq!(request.mime_type, "application/pdf");
    assert_eq!(request.bytes_base64, "pdfdata==");
    assert_eq!(request.context_id.as_str(), "ctx_123");
    assert_eq!(request.name, Some("document.pdf".to_string()));
    assert_eq!(request.user_id.as_ref().unwrap().as_str(), "user_abc");
    assert_eq!(request.session_id.as_ref().unwrap().as_str(), "sess_xyz");
    assert_eq!(request.trace_id.as_ref().unwrap().as_str(), "trace_def");
}

#[test]
fn test_file_upload_request_builder_static_method() {
    let context_id = ContextId::new("ctx_456");
    let request = FileUploadRequest::builder("image/jpeg", "jpegdata==", context_id).build();

    assert_eq!(request.mime_type, "image/jpeg");
    assert_eq!(request.bytes_base64, "jpegdata==");
}

#[test]
fn test_file_upload_request_clone() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequest::builder("image/png", "data==", context_id)
        .with_name("test.png")
        .build();

    let cloned = request.clone();
    assert_eq!(request.mime_type, cloned.mime_type);
    assert_eq!(request.bytes_base64, cloned.bytes_base64);
    assert_eq!(request.name, cloned.name);
}

#[test]
fn test_file_upload_request_debug() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequest::builder("image/png", "data==", context_id).build();

    let debug_str = format!("{:?}", request);
    assert!(debug_str.contains("FileUploadRequest"));
    assert!(debug_str.contains("image/png"));
}

#[test]
fn test_uploaded_file_struct() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id: file_id.clone(),
        path: "/storage/uploads/test.png".to_string(),
        public_url: "/files/uploads/test.png".to_string(),
        size_bytes: 4096,
    };

    assert_eq!(uploaded.file_id.as_str(), file_id.as_str());
    assert_eq!(uploaded.path, "/storage/uploads/test.png");
    assert_eq!(uploaded.public_url, "/files/uploads/test.png");
    assert_eq!(uploaded.size_bytes, 4096);
}

#[test]
fn test_uploaded_file_clone() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id,
        path: "/storage/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        size_bytes: 1024,
    };

    let cloned = uploaded.clone();
    assert_eq!(uploaded.path, cloned.path);
    assert_eq!(uploaded.public_url, cloned.public_url);
    assert_eq!(uploaded.size_bytes, cloned.size_bytes);
}

#[test]
fn test_uploaded_file_debug() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id,
        path: "/storage/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        size_bytes: 2048,
    };

    let debug_str = format!("{:?}", uploaded);
    assert!(debug_str.contains("UploadedFile"));
    assert!(debug_str.contains("/storage/test.png"));
}
