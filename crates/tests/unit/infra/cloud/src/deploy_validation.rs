//! Unit tests for deploy Dockerfile validation and services-config discovery

use systemprompt_cloud::deploy::{
    find_services_config, get_required_mcp_copy_lines, validate_dockerfile_has_mcp_binaries,
    validate_dockerfile_has_no_stale_binaries, validate_profile_dockerfile,
};
use systemprompt_models::ServicesConfig;
use tempfile::TempDir;

#[test]
fn test_find_services_config_present() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().join("services/config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.yaml"), "{}").unwrap();

    let found = find_services_config(temp.path()).unwrap();
    assert_eq!(found, config_dir.join("config.yaml"));
}

#[test]
fn test_find_services_config_missing() {
    let temp = TempDir::new().unwrap();
    let err = find_services_config(temp.path()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Services config not found.\n\nExpected at: services/config/config.yaml"
    );
}

#[test]
fn test_no_missing_binaries_for_empty_project() {
    let temp = TempDir::new().unwrap();
    let config = ServicesConfig::default();
    let missing = validate_dockerfile_has_mcp_binaries("FROM debian", temp.path(), &config);
    assert!(missing.is_empty());
}

#[test]
fn test_stale_binary_detected() {
    let temp = TempDir::new().unwrap();
    let config = ServicesConfig::default();
    let content = "COPY target/release/systemprompt /app/bin/\n\
                   COPY target/release/systemprompt-old-server /app/bin/\n";
    let stale = validate_dockerfile_has_no_stale_binaries(content, temp.path(), &config);
    assert_eq!(stale, vec!["systemprompt-old-server".to_owned()]);
}

#[test]
fn test_wildcard_copy_skips_both_checks() {
    let temp = TempDir::new().unwrap();
    let config = ServicesConfig::default();
    let content = "COPY target/release/systemprompt-* /app/bin/\n";
    assert!(validate_dockerfile_has_mcp_binaries(content, temp.path(), &config).is_empty());
    assert!(validate_dockerfile_has_no_stale_binaries(content, temp.path(), &config).is_empty());
}

#[test]
fn test_validate_profile_dockerfile_missing_file() {
    let temp = TempDir::new().unwrap();
    let dockerfile_path = temp.path().join("Dockerfile");
    let config = ServicesConfig::default();

    let err = validate_profile_dockerfile(&dockerfile_path, temp.path(), &config).unwrap_err();
    let message = err.to_string();
    assert!(message.starts_with(&format!(
        "Dockerfile not found at {}",
        dockerfile_path.display()
    )));
    assert!(message.contains("systemprompt cloud profile create"));
}

#[test]
fn test_validate_profile_dockerfile_reports_stale() {
    let temp = TempDir::new().unwrap();
    let dockerfile_path = temp.path().join("Dockerfile");
    std::fs::write(
        &dockerfile_path,
        "COPY target/release/systemprompt-old-server /app/bin/\n",
    )
    .unwrap();
    let config = ServicesConfig::default();

    let err = validate_profile_dockerfile(&dockerfile_path, temp.path(), &config).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("dev-only or removed"));
    assert!(message.contains("systemprompt-old-server"));
}

#[test]
fn test_validate_profile_dockerfile_passes_clean_file() {
    let temp = TempDir::new().unwrap();
    let dockerfile_path = temp.path().join("Dockerfile");
    std::fs::write(
        &dockerfile_path,
        "COPY target/release/systemprompt /app/bin/\n",
    )
    .unwrap();
    let config = ServicesConfig::default();

    validate_profile_dockerfile(&dockerfile_path, temp.path(), &config).unwrap();
}

#[test]
fn test_get_required_mcp_copy_lines_with_no_extensions_is_empty() {
    let temp = TempDir::new().unwrap();
    let lines = get_required_mcp_copy_lines(temp.path(), &ServicesConfig::default());
    assert!(lines.is_empty());
}

#[test]
fn test_stale_extractor_reports_each_specific_binary() {
    let temp = TempDir::new().unwrap();
    let dockerfile = "FROM rust\nCOPY target/release/systemprompt-foo /bin/\nCOPY \
                      target/release/systemprompt-bar /bin/\n";
    let stale =
        validate_dockerfile_has_no_stale_binaries(dockerfile, temp.path(), &ServicesConfig::default());
    assert_eq!(stale.len(), 2);
    assert!(stale.iter().any(|s| s == "systemprompt-foo"));
    assert!(stale.iter().any(|s| s == "systemprompt-bar"));
}

#[test]
fn test_stale_extractor_ignores_non_systemprompt_copy_lines() {
    let temp = TempDir::new().unwrap();
    let dockerfile = "FROM rust\nCOPY target/release/other-tool /bin/\nCOPY src/ /app/src/\n";
    let stale =
        validate_dockerfile_has_no_stale_binaries(dockerfile, temp.path(), &ServicesConfig::default());
    assert!(stale.is_empty());
}

#[test]
fn test_no_release_copy_lines_extracts_nothing() {
    let temp = TempDir::new().unwrap();
    let dockerfile = "FROM rust\nWORKDIR /app\n";
    let stale =
        validate_dockerfile_has_no_stale_binaries(dockerfile, temp.path(), &ServicesConfig::default());
    assert!(stale.is_empty());
    let missing =
        validate_dockerfile_has_mcp_binaries(dockerfile, temp.path(), &ServicesConfig::default());
    assert!(missing.is_empty());
}

#[test]
fn test_bare_systemprompt_wildcard_is_filtered_from_extraction() {
    let temp = TempDir::new().unwrap();
    let dockerfile = "COPY target/release/systemprompt-* /bin/\n";
    let stale =
        validate_dockerfile_has_no_stale_binaries(dockerfile, temp.path(), &ServicesConfig::default());
    assert!(stale.is_empty());
}
