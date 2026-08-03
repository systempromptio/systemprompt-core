//! Filesystem-error arms of `SchemaLoader`, distinct from the missing-path
//! arms the main suite covers: a schema path that exists but cannot be read as
//! a file, and a `schema` entry that exists but is not a directory.

use std::path::Path;

use systemprompt_mcp::services::schema::SchemaLoader;

#[test]
fn load_schema_file_reports_a_read_failure_for_a_path_that_is_a_directory() {
    let tmp = tempfile::tempdir().expect("tmp");
    std::fs::create_dir(tmp.path().join("schema.sql")).expect("directory in the file's place");

    let err = SchemaLoader::load_schema_file(tmp.path(), "schema.sql")
        .expect_err("a directory is not readable as a schema");
    assert!(
        err.to_string().contains("Failed to read schema file"),
        "an existing-but-unreadable path is a read failure, not a missing file: {err}"
    );
}

#[test]
fn list_schema_files_reports_a_read_failure_when_schema_is_not_a_directory() {
    let tmp = tempfile::tempdir().expect("tmp");
    std::fs::write(tmp.path().join("schema"), b"not a directory").expect("write blocker");

    let err = SchemaLoader::list_schema_files(tmp.path())
        .expect_err("a regular file cannot be enumerated");
    assert!(
        err.to_string().contains("Failed to read schema directory"),
        "unexpected error: {err}"
    );
}

#[test]
fn list_schema_files_ignores_subdirectories_and_non_sql_entries() {
    let tmp = tempfile::tempdir().expect("tmp");
    let schema_dir = tmp.path().join("schema");
    std::fs::create_dir(&schema_dir).expect("schema dir");
    std::fs::write(schema_dir.join("001_init.sql"), b"CREATE TABLE t (id int);").expect("sql");
    std::fs::write(schema_dir.join("README.md"), b"notes").expect("md");
    std::fs::create_dir(schema_dir.join("nested.sql")).expect("nested dir");

    let files = SchemaLoader::list_schema_files(tmp.path()).expect("list");

    let names: Vec<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert!(names.contains(&"001_init.sql".to_owned()));
    assert!(!names.contains(&"README.md".to_owned()));
}

#[test]
fn list_schema_files_of_a_service_without_a_schema_directory_is_empty() {
    let tmp = tempfile::tempdir().expect("tmp");

    assert!(
        SchemaLoader::list_schema_files(tmp.path())
            .expect("list")
            .is_empty()
    );
    assert!(
        SchemaLoader::list_schema_files(Path::new("/nonexistent/service/path"))
            .expect("list")
            .is_empty()
    );
}
