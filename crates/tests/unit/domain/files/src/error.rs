use systemprompt_files::error::FilesError;

#[test]
fn files_error_storage_variant_display() {
    let err = FilesError::Storage("disk full".to_owned());
    let s = format!("{err}");
    assert!(s.contains("storage error"));
    assert!(s.contains("disk full"));
}

#[test]
fn files_error_io_variant_display() {
    let io = std::io::Error::other("perm denied");
    let err = FilesError::from(io);
    let s = format!("{err}");
    assert!(s.contains("perm denied"));
}

#[test]
fn files_error_json_variant_display() {
    let json_err = serde_json::from_str::<i32>("nope").unwrap_err();
    let err = FilesError::from(json_err);
    let s = format!("{err}");
    assert!(s.contains("json:"));
}

#[test]
fn files_error_validation_variant_display() {
    let err = FilesError::Validation("bad input".to_owned());
    let s = format!("{err}");
    assert!(s.contains("bad input"));
}

#[test]
fn files_error_not_found_variant_display() {
    let err = FilesError::NotFound("foo".to_owned());
    let s = format!("{err}");
    assert!(s.contains("foo"));
}

#[test]
fn files_error_config_variant_display() {
    let err = FilesError::Config("not init".to_owned());
    let s = format!("{err}");
    assert!(s.contains("not init"));
}

#[test]
fn files_error_debug_contains_variant_name() {
    let err = FilesError::Storage("x".to_owned());
    let d = format!("{err:?}");
    assert!(d.contains("Storage"));
}

#[test]
fn from_sqlx_error_wraps_repository_variant() {
    let err = FilesError::from(sqlx::Error::RowNotFound);
    match &err {
        FilesError::Repository(inner) => {
            assert!(
                inner.to_string().contains("no rows returned"),
                "unexpected repository error: {inner}"
            );
        },
        other => panic!("expected Repository, got {other:?}"),
    }
}
