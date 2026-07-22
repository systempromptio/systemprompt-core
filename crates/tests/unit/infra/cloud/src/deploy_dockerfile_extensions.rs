//! Dockerfile generation and validation for a project that carries MCP
//! extensions: the COPY sections for extension assets and MCP binaries,
//! dev-only filtering through a real services config file, and the
//! missing/stale error branches of `validate_profile_dockerfile`.

use std::collections::HashMap;
use std::path::Path;

use systemprompt_cloud::deploy::{
    DockerfileBuilder, get_required_mcp_copy_lines, validate_dockerfile_has_mcp_binaries,
    validate_profile_dockerfile,
};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::Deployment;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::services::ServicesConfig;
use tempfile::TempDir;

fn write_mcp_manifest(project_root: &Path, ext_name: &str, binary: &str) {
    let dir = project_root.join("extensions").join(ext_name);
    std::fs::create_dir_all(&dir).expect("create ext dir");
    std::fs::write(
        dir.join("manifest.yaml"),
        format!(
            "extension:\n  type: mcp\n  name: {ext_name}\n  binary: {binary}\n  enabled: true\n"
        ),
    )
    .expect("write manifest");
}

fn deployment(binary: &str, dev_only: bool) -> Deployment {
    Deployment {
        server_type: McpServerType::Internal,
        binary: binary.to_owned(),
        package: None,
        port: 5001,
        endpoint: None,
        enabled: true,
        display_in_web: false,
        dev_only,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::new(),
    }
}

fn write_services_config(project_root: &Path, config: &ServicesConfig) {
    let dir = project_root.join("services/config");
    std::fs::create_dir_all(&dir).expect("create config dir");
    let yaml = serde_yaml::to_string(config).expect("serialise services config");
    std::fs::write(dir.join("config.yaml"), yaml).expect("write config");
}

#[test]
fn dockerfile_copies_extension_assets_and_mcp_binaries() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "cov-ext", "systemprompt-cov-mcp");

    let content = DockerfileBuilder::new(temp.path()).build();

    assert!(content.contains("# Copy MCP server binaries"));
    assert!(content.contains("COPY target/release/systemprompt-cov-mcp /app/bin/"));
    assert!(content.contains("# Copy extension assets"));
    assert!(content.contains("COPY extensions/cov-ext /app/extensions/cov-ext"));
}

#[test]
fn dockerfile_with_unparseable_services_config_falls_back_to_all_binaries() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "cov-ext", "systemprompt-cov-mcp");
    let dir = temp.path().join("services/config");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), ": not : valid : yaml :").unwrap();

    let content = DockerfileBuilder::new(temp.path()).build();

    assert!(content.contains("COPY target/release/systemprompt-cov-mcp /app/bin/"));
}

#[test]
fn dockerfile_with_services_config_excludes_dev_only_binaries() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "prod-ext", "systemprompt-cov-prod");
    write_mcp_manifest(temp.path(), "dev-ext", "systemprompt-cov-dev");

    let mut config = ServicesConfig::default();
    config.mcp_servers.insert(
        "dev-server".to_owned(),
        deployment("systemprompt-cov-dev", true),
    );
    write_services_config(temp.path(), &config);

    let content = DockerfileBuilder::new(temp.path()).build();

    assert!(content.contains("COPY target/release/systemprompt-cov-prod /app/bin/"));
    assert!(!content.contains("COPY target/release/systemprompt-cov-dev /app/bin/"));
}

#[test]
fn required_copy_lines_and_missing_detection_for_discovered_extension() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "cov-ext", "systemprompt-cov-mcp");
    let config = ServicesConfig::default();

    let lines = get_required_mcp_copy_lines(temp.path(), &config);
    assert_eq!(
        lines,
        vec!["COPY target/release/systemprompt-cov-mcp /app/bin/".to_owned()]
    );

    let missing = validate_dockerfile_has_mcp_binaries("FROM debian", temp.path(), &config);
    assert_eq!(missing, vec!["systemprompt-cov-mcp".to_owned()]);

    let complete = "COPY target/release/systemprompt-cov-mcp /app/bin/";
    assert!(validate_dockerfile_has_mcp_binaries(complete, temp.path(), &config).is_empty());
}

#[test]
fn validate_profile_dockerfile_reports_missing_binaries() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "cov-ext", "systemprompt-cov-mcp");
    let dockerfile_path = temp.path().join("Dockerfile");
    std::fs::write(
        &dockerfile_path,
        "COPY target/release/systemprompt /app/bin/\n",
    )
    .unwrap();

    let err =
        validate_profile_dockerfile(&dockerfile_path, temp.path(), &ServicesConfig::default())
            .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("missing COPY commands"), "got {message}");
    assert!(message.contains("systemprompt-cov-mcp"));
    assert!(message.contains("COPY target/release/systemprompt-cov-mcp /app/bin/"));
}

#[test]
fn validate_profile_dockerfile_reports_missing_and_stale_together() {
    let temp = TempDir::new().unwrap();
    write_mcp_manifest(temp.path(), "cov-ext", "systemprompt-cov-mcp");
    let dockerfile_path = temp.path().join("Dockerfile");
    std::fs::write(
        &dockerfile_path,
        "COPY target/release/systemprompt-stale-server /app/bin/\n",
    )
    .unwrap();

    let err =
        validate_profile_dockerfile(&dockerfile_path, temp.path(), &ServicesConfig::default())
            .unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("Missing binaries: systemprompt-cov-mcp"),
        "got {message}"
    );
    assert!(
        message.contains("systemprompt-stale-server"),
        "got {message}"
    );
}
