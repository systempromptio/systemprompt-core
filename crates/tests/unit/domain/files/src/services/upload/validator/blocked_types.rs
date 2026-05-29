use systemprompt_files::{FileUploadConfig, FileValidationError, FileValidator};

fn default_validator() -> FileValidator {
    FileValidator::new(FileUploadConfig::default())
}

fn assert_blocked(mime: &str) {
    let v = default_validator();
    match v.validate(mime, 1000).unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
        other => panic!("expected TypeBlocked for {mime}, got {other:?}"),
    }
}

#[test]
fn blocked_msdos_program() {
    assert_blocked("application/x-msdos-program");
}

#[test]
fn blocked_msdownload() {
    assert_blocked("application/x-msdownload");
}

#[test]
fn blocked_shellscript() {
    assert_blocked("application/x-shellscript");
}

#[test]
fn blocked_csh() {
    assert_blocked("application/x-csh");
}

#[test]
fn blocked_bash_mime() {
    assert_blocked("application/x-bash");
}

#[test]
fn blocked_bat() {
    assert_blocked("application/bat");
}

#[test]
fn blocked_x_bat() {
    assert_blocked("application/x-bat");
}

#[test]
fn blocked_msi() {
    assert_blocked("application/x-msi");
}

#[test]
fn blocked_portable_executable() {
    assert_blocked("application/vnd.microsoft.portable-executable");
}

#[test]
fn blocked_dosexec() {
    assert_blocked("application/x-dosexec");
}

#[test]
fn blocked_python_code() {
    assert_blocked("application/x-python-code");
}

#[test]
fn blocked_text_javascript() {
    assert_blocked("text/javascript");
}

#[test]
fn blocked_x_php() {
    assert_blocked("application/x-php");
}

#[test]
fn blocked_text_x_php() {
    assert_blocked("text/x-php");
}

#[test]
fn blocked_perl() {
    assert_blocked("application/x-perl");
}

#[test]
fn blocked_text_x_perl() {
    assert_blocked("text/x-perl");
}

#[test]
fn blocked_ruby() {
    assert_blocked("application/x-ruby");
}

#[test]
fn blocked_text_x_ruby() {
    assert_blocked("text/x-ruby");
}

#[test]
fn blocked_java_archive() {
    assert_blocked("application/java-archive");
}

#[test]
fn blocked_types_case_insensitive() {
    let v = default_validator();
    match v.validate("APPLICATION/X-SH", 1000).unwrap_err() {
        FileValidationError::TypeBlocked { .. } => {},
        other => panic!("expected TypeBlocked, got {other:?}"),
    }
}

#[test]
fn size_exactly_at_limit_is_allowed() {
    use systemprompt_files::{AllowedFileTypes, FilePersistenceMode};
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 1000,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes::default(),
    };
    let v = FileValidator::new(config);
    assert!(v.validate("image/png", 1000).is_ok());
}

#[test]
fn size_one_over_limit_is_rejected() {
    use systemprompt_files::{AllowedFileTypes, FilePersistenceMode};
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 1000,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes::default(),
    };
    let v = FileValidator::new(config);
    match v.validate("image/png", 1001).unwrap_err() {
        FileValidationError::FileTooLarge { size, max } => {
            assert_eq!(size, 1001);
            assert_eq!(max, 1000);
        },
        other => panic!("expected FileTooLarge, got {other:?}"),
    }
}

#[test]
fn zero_size_file_is_allowed_when_limit_is_zero() {
    use systemprompt_files::{AllowedFileTypes, FilePersistenceMode};
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 0,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes::default(),
    };
    let v = FileValidator::new(config);
    assert!(v.validate("image/png", 0).is_ok());
}

#[test]
fn images_disabled_rejects_image_category() {
    use systemprompt_files::{AllowedFileTypes, FilePersistenceMode};
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 50 * 1024 * 1024,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes {
            images: false,
            documents: true,
            audio: true,
            video: false,
        },
    };
    let v = FileValidator::new(config);
    match v.validate("image/png", 100).unwrap_err() {
        FileValidationError::CategoryDisabled { category } => {
            assert_eq!(category, "image");
        },
        other => panic!("expected CategoryDisabled, got {other:?}"),
    }
}
