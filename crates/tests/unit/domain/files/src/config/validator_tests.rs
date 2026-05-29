use systemprompt_files::FilesConfigValidator;
use systemprompt_traits::DomainConfig;

#[test]
fn domain_id_is_files() {
    let v = FilesConfigValidator::new();
    assert_eq!(v.domain_id(), "files");
}

#[test]
fn priority_returns_ten() {
    let v = FilesConfigValidator::new();
    assert_eq!(v.priority(), 10);
}

#[test]
fn validate_without_load_returns_error() {
    let v = FilesConfigValidator::new();
    let result = v.validate();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Not loaded") || !err.is_empty());
}

#[test]
fn default_constructs_same_as_new() {
    let a = FilesConfigValidator::new();
    let b = FilesConfigValidator::default();
    assert_eq!(a.domain_id(), b.domain_id());
    assert_eq!(a.priority(), b.priority());
}

#[test]
fn debug_format_contains_struct_name() {
    let v = FilesConfigValidator::new();
    let s = format!("{v:?}");
    assert!(s.contains("FilesConfigValidator"));
}
