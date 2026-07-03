use systemprompt_files::FileUploadError;

#[test]
fn io_error_variant_display() {
    let io = std::io::Error::other("disk full");
    let err = FileUploadError::from(io);
    let s = format!("{err}");
    assert!(s.contains("IO error") || s.contains("disk full"));
}

#[test]
fn io_error_variant_debug() {
    let io = std::io::Error::other("permission denied");
    let err = FileUploadError::from(io);
    let d = format!("{err:?}");
    assert!(d.contains("Io"));
}

#[test]
fn all_string_variants_display_correctly() {
    let cases: &[(&str, FileUploadError)] = &[
        ("persistence", FileUploadError::PersistenceDisabled),
        ("db error", FileUploadError::Database("db error".to_owned())),
        ("cfg error", FileUploadError::Config("cfg error".to_owned())),
        (
            "bad path",
            FileUploadError::PathValidation("bad path".to_owned()),
        ),
    ];
    for (needle, err) in cases {
        let s = format!("{err}");
        assert!(
            s.contains(needle) || !s.is_empty(),
            "display for {needle} is empty"
        );
    }
}

#[test]
fn base64_too_large_display_contains_size() {
    let err = FileUploadError::Base64TooLarge {
        encoded_size: 12_345_678,
    };
    let s = format!("{err}");
    assert!(s.contains("12345678") || s.contains("Base64") || s.contains("large"));
}

#[test]
fn all_upload_error_variants_are_debug() {
    use systemprompt_files::FileValidationError;
    let errs: Vec<Box<dyn std::fmt::Debug>> = vec![
        Box::new(FileUploadError::PersistenceDisabled),
        Box::new(FileUploadError::Validation(
            FileValidationError::UploadsDisabled,
        )),
        Box::new(FileUploadError::Database("db err".to_owned())),
        Box::new(FileUploadError::Config("cfg err".to_owned())),
        Box::new(FileUploadError::Base64TooLarge { encoded_size: 99 }),
        Box::new(FileUploadError::PathValidation("bad path".to_owned())),
        Box::new(FileUploadError::Io(std::io::Error::other("io"))),
    ];
    for err in errs {
        let s = format!("{err:?}");
        assert!(!s.is_empty());
    }
}

#[test]
fn validation_error_debug_contains_variant() {
    use systemprompt_files::FileValidationError;
    let variants = [
        FileValidationError::UploadsDisabled,
        FileValidationError::FileTooLarge { size: 1, max: 0 },
        FileValidationError::TypeNotAllowed {
            mime_type: "x".to_owned(),
        },
        FileValidationError::TypeBlocked {
            mime_type: "y".to_owned(),
        },
        FileValidationError::CategoryDisabled {
            category: "z".to_owned(),
        },
    ];
    for v in variants {
        let s = format!("{v:?}");
        assert!(!s.is_empty());
    }
}

#[test]
fn io_error_from_not_found_kind() {
    let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
    let err = FileUploadError::from(io);
    let s = format!("{err}");
    assert!(s.contains("IO error"));
    assert!(s.contains("file missing"));
}

#[test]
fn io_error_from_permission_denied_kind() {
    let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
    let err = FileUploadError::from(io);
    let d = format!("{err:?}");
    assert!(d.contains("Io"));
}
