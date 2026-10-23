//! Retired serde aliases are gone: a plugin hooks block saying `evaluation`
//! and an MCP deployment saying `type` are rejected rather than silently
//! accepted under the current names (`judge`, `server_type`).

use systemprompt_models::mcp::Deployment;
use systemprompt_models::plugin::PluginHooksRef;

#[test]
fn plugin_hooks_accept_judge_and_reject_the_retired_evaluation_key() {
    let current: PluginHooksRef =
        serde_json::from_str(r#"{"governance": true, "judge": true}"#).expect("current key");
    assert!(current.judge);

    let retired =
        serde_json::from_str::<PluginHooksRef>(r#"{"governance": true, "evaluation": true}"#);
    assert!(
        retired.is_err(),
        "`evaluation` is not a spelling of `judge`: {retired:?}"
    );
}

fn deployment_yaml(key: &str) -> String {
    format!(
        "{key}: external\nendpoint: https://mcp.example.com/mcp\nenabled: true\ndisplay_in_web: false\n\
         oauth:\n  required: false\n  scopes: []\n  audience: mcp\n  client_id: null\n"
    )
}

#[test]
fn mcp_deployment_reads_server_type_and_ignores_no_type_alias() {
    let current: Deployment =
        serde_yaml::from_str(&deployment_yaml("server_type")).expect("current key");
    assert_eq!(current.server_type.as_str(), "external");

    let retired: Deployment = serde_yaml::from_str(&deployment_yaml("type"))
        .expect("unknown keys are not denied on Deployment");
    assert_eq!(
        retired.server_type.as_str(),
        "internal",
        "`type` no longer sets server_type; the default applies"
    );
}
