//! Coverage for the offline extension-config preflight.
//!
//! `systemprompt_extension::AdmittedConfigs::validate` is the shared validator
//! the serve boot path and the `cloud deploy` / `cloud doctor` preflight both
//! run over the admitted build. A `slack.yaml` in the flat shape rather than
//! `map<app-name, SlackAppConfig>` must be reported as an error here rather
//! than first crashing the deployed binary.

use std::fs;

use systemprompt_extension::admitted::{AdmittedMember, AdmittedParts};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Admitted, Contributions, Extension, StandingTag};
use systemprompt_identifiers::ExtensionId;
use systemprompt_slack_entry::SlackExtension;
use tempfile::TempDir;

fn slack_admitted() -> Admitted {
    let extension = SlackExtension;
    let metadata = extension.metadata();
    let owner = ExtensionId::new(metadata.id);
    let mut sink = ContributionSink::default();
    extension
        .contribute(&mut Contributions::__new(owner.clone(), &mut sink))
        .expect("the slack member contributes");
    Admitted::__from_facade(AdmittedParts {
        members: vec![AdmittedMember {
            secret_grants: Vec::new(),
            owner,
            metadata,
            standing: StandingTag::Core,
            sink,
        }],
        ..AdmittedParts::default()
    })
}

fn services_dir_with_slack(slack_yaml: &str) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let config_dir = dir.path().join("config");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::write(config_dir.join("slack.yaml"), slack_yaml).expect("write slack.yaml");
    dir
}

fn slack_error(dir: &std::path::Path) -> Option<String> {
    let outcomes = systemprompt_extension::AdmittedConfigs::of(slack_admitted().built()).validate(dir);
    let slack = outcomes
        .iter()
        .find(|o| o.extension_id == "slack")
        .expect("the admitted slack member declares its config");
    slack.error.as_ref().map(ToString::to_string)
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
