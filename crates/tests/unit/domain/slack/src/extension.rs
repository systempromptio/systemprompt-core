//! Tests for the Slack extension registration surface.

use systemprompt_extension::Extension;
use systemprompt_slack::SlackExtension;

#[test]
fn config_schema_describes_a_map_of_slack_app_configs() {
    let schema = SlackExtension
        .config_schema()
        .expect("slack advertises a config schema");

    let rendered = schema.to_string();
    assert!(
        rendered.contains("SlackAppConfig"),
        "schema should reference SlackAppConfig, got: {rendered}"
    );
    assert!(
        rendered.contains("workspace_id"),
        "schema should expose the per-app workspace_id field, got: {rendered}"
    );
}

#[test]
fn metadata_and_prefix_identify_the_slack_extension() {
    let meta = SlackExtension.metadata();
    assert_eq!(meta.id, "slack");
    assert_eq!(SlackExtension.config_prefix(), Some("slack"));
}
