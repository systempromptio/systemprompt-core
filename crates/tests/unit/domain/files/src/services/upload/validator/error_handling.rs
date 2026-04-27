//! Unit tests for FileValidator error handling (disabled, too large, blocked,
//! etc.)

use systemprompt_files::{AllowedFileTypes, FileUploadConfig, FileValidationError, FileValidator};

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

fn default_config() -> FileUploadConfig {
    FileUploadConfig::default()
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

#[test]
fn test_file_validator_uploads_disabled() {
    let validator = FileValidator::new(disabled_config());
    let result = validator.validate("image/png", 1000);
    match result.unwrap_err() {
        FileValidationError::UploadsDisabled => {},
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
        },
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
        },
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
        },
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_shell_script() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-sh", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_javascript() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/javascript", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_php() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-httpd-php", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
        _ => panic!("Expected TypeBlocked error"),
    }
}

#[test]
fn test_file_validator_blocked_java_class() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/x-java-class", 1000);
    match result.unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
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
        },
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
        },
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
        },
        _ => panic!("Expected CategoryDisabled error"),
    }
}
