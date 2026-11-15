//! Isolated lifecycle coverage for the profile-driven default model: a
//! missing catalog is seeded, an existing one only has its
//! `gateway.default_model` moved to the chosen vendor.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::Command;

use systemprompt_cli::cloud::profile::templates::update_ai_config_default_provider;

const HELPER: &str = "commands::cloud_profile_ai_config_lifecycle::ai_config_lifecycle_helper";

fn gateway(root: &std::path::Path) -> serde_yaml::Value {
    let text = std::fs::read_to_string(root.join("ai/gateway.yaml")).expect("read gateway.yaml");
    serde_yaml::from_str(&text).expect("valid gateway YAML")
}

#[test]
#[ignore = "re-executed with an isolated services path by the parent test"]
fn ai_config_lifecycle_helper() {
    let root = std::env::var("SYSTEMPROMPT_SERVICES_PATH")
        .expect("the parent supplies an isolated services root");
    let root = std::path::Path::new(&root);

    update_ai_config_default_provider("anthropic").expect("seed the missing catalog");
    assert!(root.join("ai/config.yaml").exists());
    assert!(root.join("ai/providers.yaml").exists());
    let created = gateway(root);
    let model = created["gateway"]["default_model"]
        .as_str()
        .expect("default model");
    assert!(model.starts_with("claude-"), "{created:?}");

    let providers_before = std::fs::read_to_string(root.join("ai/providers.yaml")).unwrap();
    update_ai_config_default_provider("openai").expect("move the default model");
    let updated = gateway(root);
    let model = updated["gateway"]["default_model"]
        .as_str()
        .expect("default model");
    assert!(!model.starts_with("claude-"), "{updated:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("ai/providers.yaml")).unwrap(),
        providers_before,
        "moving the default model must not rewrite the providers"
    );
}

#[test]
fn ai_config_is_seeded_then_its_default_model_moves() {
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
    assert!(gateway(services.path())["gateway"]["default_model"].is_string());
}
