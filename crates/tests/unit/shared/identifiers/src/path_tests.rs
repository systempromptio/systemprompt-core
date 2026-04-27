use systemprompt_identifiers::{DbValue, ToDbValue, ValidatedFilePath};

#[test]
fn valid_simple_path() {
    let path = ValidatedFilePath::try_new("documents/file.txt").unwrap();
    assert_eq!(path.as_str(), "documents/file.txt");
}

#[test]
fn valid_absolute_path() {
    let path = ValidatedFilePath::try_new("/var/www/html/index.html").unwrap();
    assert_eq!(path.as_str(), "/var/www/html/index.html");
}

#[test]
fn valid_windows_path() {
    let path = ValidatedFilePath::try_new("C:\\Users\\test\\file.txt").unwrap();
    assert_eq!(path.as_str(), "C:\\Users\\test\\file.txt");
}

#[test]
fn valid_single_dot_component() {
    let path = ValidatedFilePath::try_new("./file.txt").unwrap();
    assert_eq!(path.as_str(), "./file.txt");
}

#[test]
fn rejects_empty_string() {
    let err = ValidatedFilePath::try_new("").unwrap_err();
    assert_eq!(err.to_string(), "ValidatedFilePath cannot be empty");
}

#[test]
fn rejects_null_byte() {
    let err = ValidatedFilePath::try_new("path/to\0file").unwrap_err();
    assert!(err.to_string().contains("null bytes"));
}

#[test]
fn rejects_dot_dot_traversal() {
    let err = ValidatedFilePath::try_new("path/../etc/passwd").unwrap_err();
    assert!(err.to_string().contains("path traversal"));
}

#[test]
fn rejects_dot_dot_at_start() {
    let err = ValidatedFilePath::try_new("../secret").unwrap_err();
    assert!(err.to_string().contains("path traversal"));
}

#[test]
fn rejects_dot_dot_at_end() {
    let err = ValidatedFilePath::try_new("path/..").unwrap_err();
    assert!(err.to_string().contains("path traversal"));
}

#[test]
fn rejects_dot_dot_backslash_traversal() {
    let err = ValidatedFilePath::try_new("path\\..\\etc\\passwd").unwrap_err();
    assert!(err.to_string().contains("path traversal"));
}

#[test]
fn rejects_encoded_traversal_percent_2e_2e() {
    let err = ValidatedFilePath::try_new("path/%2e%2e/secret").unwrap_err();
    assert!(err.to_string().contains("encoded path traversal"));
}

#[test]
fn rejects_encoded_traversal_mixed_dot_percent() {
    let err = ValidatedFilePath::try_new("path/%2e./secret").unwrap_err();
    assert!(err.to_string().contains("encoded path traversal"));
}

#[test]
fn rejects_encoded_traversal_dot_percent() {
    let err = ValidatedFilePath::try_new("path/.%2e/secret").unwrap_err();
    assert!(err.to_string().contains("encoded path traversal"));
}

#[test]
fn rejects_double_encoded_traversal() {
    let err = ValidatedFilePath::try_new("path/%252e%252e/secret").unwrap_err();
    assert!(err.to_string().contains("double-encoded"));
}

#[test]
fn extension_extraction_simple() {
    let path = ValidatedFilePath::new("file.txt");
    assert_eq!(path.extension(), Some("txt"));
}

#[test]
fn extension_extraction_nested() {
    let path = ValidatedFilePath::new("/path/to/file.tar.gz");
    assert_eq!(path.extension(), Some("gz"));
}

#[test]
fn extension_none_for_no_extension() {
    let path = ValidatedFilePath::new("Makefile");
    assert_eq!(path.extension(), None);
}

#[test]
fn extension_none_for_trailing_dot() {
    let path = ValidatedFilePath::new("file.");
    assert_eq!(path.extension(), None);
}

#[test]
fn file_name_extraction() {
    let path = ValidatedFilePath::new("/path/to/file.txt");
    assert_eq!(path.file_name(), Some("file.txt"));
}

#[test]
fn file_name_from_windows_path() {
    let path = ValidatedFilePath::new("C:\\Users\\test\\file.txt");
    assert_eq!(path.file_name(), Some("file.txt"));
}

#[test]
fn file_name_simple() {
    let path = ValidatedFilePath::new("file.txt");
    assert_eq!(path.file_name(), Some("file.txt"));
}

#[test]
fn display_shows_full_path() {
    let path = ValidatedFilePath::new("/var/www/file.txt");
    assert_eq!(format!("{}", path), "/var/www/file.txt");
}

#[test]
fn serde_roundtrip_exact_json() {
    let path = ValidatedFilePath::new("documents/file.txt");
    let json = serde_json::to_string(&path).unwrap();
    assert_eq!(json, "\"documents/file.txt\"");
    let deserialized: ValidatedFilePath = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, path);
}

#[test]
fn serde_rejects_traversal_on_deserialize() {
    let result: Result<ValidatedFilePath, _> = serde_json::from_str("\"../etc/passwd\"");
    assert!(result.is_err());
}

#[test]
fn try_from_str_ref() {
    let path: ValidatedFilePath = "valid/path.txt".try_into().unwrap();
    assert_eq!(path.as_str(), "valid/path.txt");
}

#[test]
fn try_from_string() {
    let path: ValidatedFilePath = String::from("valid/path.txt").try_into().unwrap();
    assert_eq!(path.as_str(), "valid/path.txt");
}

#[test]
fn from_str_parse() {
    let path: ValidatedFilePath = "valid/path.txt".parse().unwrap();
    assert_eq!(path.as_str(), "valid/path.txt");
}

#[test]
fn to_db_value_returns_string_variant() {
    let path = ValidatedFilePath::new("file.txt");
    let db_val = path.to_db_value();
    assert!(matches!(db_val, DbValue::String(s) if s == "file.txt"));
}

#[test]
#[should_panic(expected = "ValidatedFilePath validation failed")]
fn new_panics_on_traversal() {
    let _ = ValidatedFilePath::new("../secret");
}

#[test]
fn equality_across_construction_paths() {
    let from_new = ValidatedFilePath::new("file.txt");
    let from_try: ValidatedFilePath = "file.txt".try_into().unwrap();
    let from_parse: ValidatedFilePath = "file.txt".parse().unwrap();
    assert_eq!(from_new, from_try);
    assert_eq!(from_try, from_parse);
}
