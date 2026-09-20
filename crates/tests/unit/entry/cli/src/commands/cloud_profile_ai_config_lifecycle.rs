//! Isolated lifecycle coverage for profile-driven AI provider configuration.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::Command;

use systemprompt_cli::cloud::profile::templates::update_ai_config_default_provider;

const HELPER: &str = "commands::cloud_profile_ai_config_lifecycle::ai_config_lifecycle_helper";

#[test]
#[ignore = "re-executed with an isolated services path by the parent test"]
fn ai_config_lifecycle_helper() {
    let root = std::env::var("SYSTEMPROMPT_SERVICES_PATH")
        .expect("the parent supplies an isolated services root");
    let config = std::path::Path::new(&root).join("ai/config.yaml");

    update_ai_config_default_provider("anthropic").expect("create the missing AI config");
    let created = std::fs::read_to_string(&config).expect("read created AI config");
    let created_yaml: serde_yaml::Value =
        serde_yaml::from_str(&created).expect("valid created YAML");
    assert_eq!(
        created_yaml["default_provider"].as_str(),
        Some("anthropic"),
        "{created}"
    );

    std::fs::write(
        &config,
        "default_provider: \"anthropic\"\nproviders:\n  anthropic:\n    enabled: true\n",
    )
    .expect("replace with a minimal existing config");
    update_ai_config_default_provider("openai").expect("update the existing AI config");

    let updated = std::fs::read_to_string(&config).expect("read updated AI config");
    let updated_yaml: serde_yaml::Value =
        serde_yaml::from_str(&updated).expect("valid updated YAML");
    assert_eq!(
        updated_yaml["default_provider"].as_str(),
        Some("openai"),
        "{updated}"
    );
    assert_eq!(
        updated_yaml["providers"]["anthropic"]["enabled"].as_bool(),
        Some(true),
        "updating the provider must preserve the rest of the config: {updated}"
    );
}

#[test]
fn ai_config_is_created_then_updated_without_losing_existing_provider_configuration() {
    let services = tempfile::tempdir().expect("isolated services root");
    let output = Command::new(std::env::current_exe().expect("unit-test binary path"))
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .env("SYSTEMPROMPT_SERVICES_PATH", services.path())
        .output()
        .expect("re-execute the lifecycle helper");

    assert!(
        output.status.success(),
        "AI config helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let config = std::fs::read_to_string(services.path().join("ai/config.yaml"))
        .expect("helper produced the AI config");
    assert!(config.contains("default_provider: \"openai\""), "{config}");
    assert!(config.contains("anthropic:"), "{config}");
}
