//! Unit tests for FileValidator image and document MIME type validation

use systemprompt_files::{FileCategory, FileUploadConfig, FileValidator};

fn default_config() -> FileUploadConfig {
    FileUploadConfig::default()
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
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_jpeg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/jpeg", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_gif() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/gif", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_webp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/webp", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_svg() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/svg+xml", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_bmp() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/bmp", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_tiff() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/tiff", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/x-icon", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_image_microsoft_icon() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("image/vnd.microsoft.icon", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}

#[test]
fn test_file_validator_validate_document_pdf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/pdf", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_word() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/msword", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_docx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        1000,
    );
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_excel() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-excel", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_xlsx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        1000,
    );
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_powerpoint() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/vnd.ms-powerpoint", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_pptx() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate(
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        1000,
    );
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_text() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/plain", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_csv() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/csv", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_markdown() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/markdown", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_html() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/html", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_json() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/json", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/xml", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_text_xml() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("text/xml", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_validate_document_rtf() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("application/rtf", 1000);
    assert_eq!(
        result.expect("validation should pass"),
        FileCategory::Document
    );
}

#[test]
fn test_file_validator_case_insensitive() {
    let validator = FileValidator::new(default_config());
    let result = validator.validate("IMAGE/PNG", 1000);
    assert_eq!(result.expect("validation should pass"), FileCategory::Image);
}
