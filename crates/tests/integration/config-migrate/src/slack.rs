//! `config migrate` renames a Slack app's retired
//! `authz.link_by_workspace_email` to `authz.link.link_by_email`, keeps the
//! operator's value and comments, and leaves current files alone.

use std::path::Path;

use systemprompt_cli::commands::admin::config::migrate::slack::{
    migrate, pending, rewrite_slack_link_key,
};

const RETIRED: &str = "\
slack_apps:
  support:
    workspace_id: \"T1\"
    authz:
      allowed_roles: [\"user\"]
      link_by_workspace_email: true # needs users:read.email
  sales:
    workspace_id: \"T2\"
    authz:
      link_by_workspace_email: false
agents:
  link_by_workspace_email: not-slack
";

const MIGRATED: &str = "\
slack_apps:
  support:
    workspace_id: \"T1\"
    authz:
      allowed_roles: [\"user\"]
      link:
        link_by_email: true # needs users:read.email
  sales:
    workspace_id: \"T2\"
    authz:
      link:
        link_by_email: false
agents:
  link_by_workspace_email: not-slack
";

fn write(root: &Path, rel: &str, body: &str) -> std::path::PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("write");
    path
}

#[test]
fn the_retired_key_becomes_the_link_policy_with_the_operators_value() {
    let root = tempfile::tempdir().expect("tempdir");
    let old = write(root.path(), "slack/apps.yaml", RETIRED);
    let current = write(
        root.path(),
        "config.yaml",
        "slack_apps:\n  ops:\n    authz:\n      link:\n        link_by_email: true\n",
    );
    assert!(pending(root.path()));

    let moved = migrate(root.path()).expect("migrate");

    assert_eq!(moved, vec![old.clone()]);
    assert_eq!(std::fs::read_to_string(old).expect("read"), MIGRATED);
    assert_eq!(
        std::fs::read_to_string(current).expect("read"),
        "slack_apps:\n  ops:\n    authz:\n      link:\n        link_by_email: true\n"
    );
    assert!(!pending(root.path()), "a second run finds nothing");
}

#[test]
fn the_migrated_authz_block_loads_through_the_slack_config_type() {
    let authz = "allowed_roles: [\"user\"]\nlink:\n  link_by_email: true\n";
    let parsed: systemprompt_manifest::services::SlackAuthzConfig =
        serde_yaml::from_str(authz).expect("migrated authz loads");
    assert!(parsed.link.link_by_email);

    let retired = "link_by_workspace_email: true\n";
    assert!(
        serde_yaml::from_str::<systemprompt_manifest::services::SlackAuthzConfig>(retired).is_err(),
        "the retired key is refused, so the migration is required"
    );
}

#[test]
fn an_authz_block_that_already_declares_link_is_left_alone() {
    let both = "slack_apps:\n  a:\n    authz:\n      link:\n        auto_provision: false\n      link_by_workspace_email: true\n";
    assert_eq!(rewrite_slack_link_key(both), None);
}
