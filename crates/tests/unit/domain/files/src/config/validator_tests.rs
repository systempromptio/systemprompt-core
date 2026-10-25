use systemprompt_files::FilesConfigValidator;
use systemprompt_manifest::validators::DomainConfig;
use systemprompt_test_fixtures::ensure_test_bootstrap;

#[test]
fn domain_id_is_files() {
    let v = FilesConfigValidator::new(ensure_test_bootstrap().app_paths.clone());
    assert_eq!(v.domain_id(), "files");
}

#[test]
fn priority_returns_ten() {
    let v = FilesConfigValidator::new(ensure_test_bootstrap().app_paths.clone());
    assert_eq!(v.priority(), 10);
}

#[test]
fn validate_without_load_returns_error() {
    let v = FilesConfigValidator::new(ensure_test_bootstrap().app_paths.clone());
    let result = v.validate();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Not loaded") || !err.is_empty());
}
