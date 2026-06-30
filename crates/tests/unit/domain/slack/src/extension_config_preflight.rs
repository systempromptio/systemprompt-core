//! Regression coverage for the offline extension-config preflight.
//!
//! `systemprompt_runtime::validate_extension_configs` is the shared validator
//! the serve boot path and the `cloud deploy` / `cloud doctor` preflight both
//! run. These tests pin the 0.17.0 incident: a `slack.yaml` left in the old
//! flat shape (after the schema moved to `map<app-name, SlackAppConfig>`) must
//! be reported as an error here rather than first crashing the deployed binary.

use std::fs;

use systemprompt_runtime::validate_extension_configs;
use tempfile::TempDir;

fn services_dir_with_slack(slack_yaml: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let config_dir = dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::write(config_dir.join("slack.yaml"), slack_yaml).expect("write slack.yaml");
    dir
}

fn slack_error(dir: &std::path::Path) -> Option<String> {
    let outcomes = validate_extension_configs(dir).expect("extension discovery");
    let slack = outcomes
        .iter()
        .find(|o| o.extension_id == "slack")
        .expect("slack extension registered in this binary");
    slack.error.clone()
}

#[test]
fn old_flat_shape_is_rejected() {
    let dir = services_dir_with_slack("default_channel_id: \"C0AGTR9TSPR\"\n");
    let error = slack_error(dir.path()).expect("old flat slack config should fail validation");
    assert!(
        error.contains("SlackAppConfig"),
        "error should name the expected struct, got: {error}"
    );
}

#[test]
fn valid_map_shape_passes() {
    let yaml = "\
my-workspace:
  workspace_id: \"T012AB3CD\"
  signing_secret_ref: \"slack_signing_secret\"
  bot_token_ref: \"slack_bot_token\"
  default_agent: \"support-agent\"
";
    let dir = services_dir_with_slack(yaml);
    assert_eq!(slack_error(dir.path()), None);
}

#[test]
fn missing_config_file_passes() {
    let dir = TempDir::new().expect("tempdir");
    assert_eq!(slack_error(dir.path()), None);
}
