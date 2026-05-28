use systemprompt_cli::cloud::dockerfile::{
    DockerfileBuilder, generate_dockerfile_content, get_required_mcp_copy_lines,
    validate_dockerfile_has_mcp_binaries, validate_dockerfile_has_no_stale_binaries,
    validate_profile_dockerfile,
};
use systemprompt_models::ServicesConfig;
use tempfile::tempdir;

fn empty_services_config() -> ServicesConfig {
    ServicesConfig::default()
}

#[test]
fn get_required_mcp_copy_lines_with_no_extensions_is_empty() {
    let root = tempdir().unwrap();
    let lines = get_required_mcp_copy_lines(root.path(), &empty_services_config());
    assert!(lines.is_empty());
}

#[test]
fn validate_dockerfile_with_wildcard_skips_all_checks() {
    let root = tempdir().unwrap();
    let dockerfile = "FROM rust\nCOPY target/release/systemprompt-* /bin/\n";
    let missing =
        validate_dockerfile_has_mcp_binaries(dockerfile, root.path(), &empty_services_config());
    let stale = validate_dockerfile_has_no_stale_binaries(
        dockerfile,
        root.path(),
        &empty_services_config(),
    );
    assert!(missing.is_empty());
    assert!(stale.is_empty());
}

#[test]
fn validate_dockerfile_extracts_specific_binary_names() {
    let root = tempdir().unwrap();
    let dockerfile = "FROM rust\nCOPY target/release/systemprompt-foo /bin/\nCOPY \
                      target/release/systemprompt-bar /bin/\n";
    let stale = validate_dockerfile_has_no_stale_binaries(
        dockerfile,
        root.path(),
        &empty_services_config(),
    );
    assert_eq!(stale.len(), 2);
    assert!(stale.iter().any(|s| s == "systemprompt-foo"));
    assert!(stale.iter().any(|s| s == "systemprompt-bar"));
}

#[test]
fn validate_dockerfile_ignores_non_systemprompt_copy_lines() {
    let root = tempdir().unwrap();
    let dockerfile = "FROM rust\nCOPY target/release/other-tool /bin/\nCOPY src/ /app/src/\n";
    let stale = validate_dockerfile_has_no_stale_binaries(
        dockerfile,
        root.path(),
        &empty_services_config(),
    );
    assert!(stale.is_empty());
}

#[test]
fn validate_profile_dockerfile_missing_file_errors() {
    let root = tempdir().unwrap();
    let missing_path = root.path().join("Dockerfile");
    let err = validate_profile_dockerfile(&missing_path, root.path(), &empty_services_config())
        .expect_err("expected error for missing dockerfile");
    assert!(err.to_string().contains("Dockerfile not found"));
}

#[test]
fn validate_profile_dockerfile_passing_file_ok() {
    let root = tempdir().unwrap();
    let dockerfile_path = root.path().join("Dockerfile");
    std::fs::write(
        &dockerfile_path,
        "FROM rust\nCOPY target/release/systemprompt-* /bin/\n",
    )
    .unwrap();
    validate_profile_dockerfile(&dockerfile_path, root.path(), &empty_services_config())
        .expect("wildcard dockerfile should pass");
}

#[test]
fn dockerfile_builder_renders_runtime_stage() {
    let root = tempdir().unwrap();
    let content = DockerfileBuilder::new(root.path())
        .with_profile("dev")
        .build();
    assert!(content.contains("FROM"));
    assert!(content.contains("systemprompt"));
}

#[test]
fn generate_dockerfile_content_includes_base_image() {
    let root = tempdir().unwrap();
    let content = generate_dockerfile_content(root.path());
    assert!(content.to_lowercase().contains("from"));
}

#[test]
fn validate_dockerfile_no_release_copy_lines_extracts_nothing() {
    let root = tempdir().unwrap();
    let dockerfile = "FROM rust\nWORKDIR /app\n";
    let stale = validate_dockerfile_has_no_stale_binaries(
        dockerfile,
        root.path(),
        &empty_services_config(),
    );
    assert!(stale.is_empty());
    let missing =
        validate_dockerfile_has_mcp_binaries(dockerfile, root.path(), &empty_services_config());
    assert!(missing.is_empty());
}

#[test]
fn validate_dockerfile_skips_bare_systemprompt_wildcard_only() {
    let root = tempdir().unwrap();
    // Confirm `systemprompt-*` line in the binary-name extractor is filtered.
    let dockerfile = "COPY target/release/systemprompt-* /bin/\n";
    let stale = validate_dockerfile_has_no_stale_binaries(
        dockerfile,
        root.path(),
        &empty_services_config(),
    );
    assert!(stale.is_empty());
}
