//! Unit tests for FileValidator audio and video MIME type validation

use systemprompt_files::{AllowedFileTypes, FileCategory, FileUploadConfig, FileValidator};

fn default_config() -> FileUploadConfig {
    FileUploadConfig::default()
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
