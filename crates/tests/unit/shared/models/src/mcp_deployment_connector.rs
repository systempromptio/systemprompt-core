//! Connector validation and the serde defaults that fill in an MCP deployment
//! YAML's omitted fields.
//!
//! The connector rules decide whether an outbound personal-account OAuth
//! configuration is coherent before anything tries to use it, and the defaults
//! decide what an operator gets when they leave a block out of the YAML
//! entirely. Both are only reachable through deserialisation and `validate`.

use std::collections::HashMap;

use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::capabilities::ToolVisibility;
use systemprompt_models::mcp::deployment::{ConnectorConfig, ToolMetadata, ToolUiConfig};
use systemprompt_models::mcp::{Deployment, McpServerType, OAuthRequirement, Settings};

fn deployment(endpoint: Option<&str>) -> Deployment {
    Deployment {
        connector: None,
        server_type: McpServerType::External,
        binary: "bin".to_owned(),
        package: None,
        port: 5100,
        endpoint: endpoint.map(str::to_owned),
        enabled: true,
        display_in_web: false,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: HashMap::new(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::new(),
    }
}

fn connector() -> ConnectorConfig {
    ConnectorConfig {
        adapter: "generic".to_owned(),
        scopes: vec!["offline_access".to_owned()],
        authorization_origins: vec!["https://idp.example.com".to_owned()],
        client_id_secret: Some("acme_client_id".to_owned()),
        client_secret: Some("acme_client_secret".to_owned()),
    }
}

#[test]
fn a_connector_on_an_https_resource_validates() {
    let mut d = deployment(Some("https://example.com/mcp"));
    d.connector = Some(connector());

    d.validate("acme")
        .expect("a generic connector over HTTPS is the supported shape");
}

#[test]
fn a_connector_over_plain_http_is_rejected() {
    let mut d = deployment(Some("http://example.com/mcp"));
    d.connector = Some(connector());

    let msg = d.validate("acme").unwrap_err().to_string();
    assert!(
        msg.contains("HTTPS") && msg.contains("acme"),
        "a connector carrying user credentials must refuse a cleartext resource: {msg}"
    );
}

#[test]
fn a_connector_without_an_endpoint_is_rejected() {
    let mut d = deployment(None);
    d.connector = Some(connector());

    let msg = d.validate("acme").unwrap_err().to_string();
    assert!(
        msg.contains("HTTPS resource"),
        "there is no resource to authorise against without an endpoint: {msg}"
    );
}

#[test]
fn an_unknown_connector_adapter_is_rejected() {
    let mut d = deployment(Some("https://example.com/mcp"));
    let mut c = connector();
    c.adapter = "salesforce".to_owned();
    d.connector = Some(c);

    let msg = d.validate("acme").unwrap_err().to_string();
    assert!(
        msg.contains("generic connector"),
        "only the generic adapter is implemented: {msg}"
    );
}

#[test]
fn a_connector_client_secret_without_a_client_id_is_rejected() {
    let mut d = deployment(Some("https://example.com/mcp"));
    let mut c = connector();
    c.client_id_secret = None;
    d.connector = Some(c);

    let msg = d.validate("acme").unwrap_err().to_string();
    assert!(
        msg.contains("client ID"),
        "half an OAuth client credential is a misconfiguration, not a public client: {msg}"
    );
}

#[test]
fn a_connector_with_neither_client_credential_validates() {
    let mut d = deployment(Some("https://example.com/mcp"));
    let mut c = connector();
    c.client_id_secret = None;
    c.client_secret = None;
    d.connector = Some(c);

    d.validate("acme")
        .expect("a public client declares no secret at all and is legitimate");
}

#[test]
fn a_connector_on_an_internal_server_is_rejected() {
    let mut d = deployment(Some("/api/v1/mcp/acme/mcp"));
    d.server_type = McpServerType::Internal;
    d.connector = Some(connector());

    let msg = d.validate("acme").unwrap_err().to_string();
    assert!(
        msg.contains("external servers"),
        "internal servers are reached with the systemprompt credential: {msg}"
    );
}

#[test]
fn an_omitted_connector_adapter_deserialises_as_generic() {
    let c: ConnectorConfig = serde_yaml::from_str("{}").expect("an empty connector block parses");

    assert_eq!(c.adapter, "generic");
    assert!(c.scopes.is_empty());
    assert!(c.authorization_origins.is_empty());
    assert!(c.client_id_secret.is_none() && c.client_secret.is_none());
}

#[test]
fn an_unknown_connector_key_is_refused_rather_than_silently_dropped() {
    let err = serde_yaml::from_str::<ConnectorConfig>("adaptor: generic\n")
        .expect_err("a misspelled key must not be ignored");

    assert!(
        err.to_string().contains("adaptor"),
        "the operator needs the offending key named: {err}"
    );
}

#[test]
fn an_omitted_tool_ui_block_defaults_to_the_artifact_uri_and_both_surfaces() {
    let ui: ToolUiConfig = serde_yaml::from_str("{}").expect("an empty ui block parses");

    assert_eq!(ui.resource_uri_template, "ui://systemprompt/{artifact_id}");
    assert_eq!(
        ui.visibility,
        vec![ToolVisibility::Model, ToolVisibility::App],
        "a tool with a UI is offered to the model and the app unless narrowed"
    );
}

#[test]
fn a_declared_tool_ui_block_overrides_the_defaults() {
    let ui: ToolUiConfig = serde_yaml::from_str(
        "resource_uri_template: \"ui://acme/{artifact_id}\"\nvisibility: \
                              [app]\n",
    )
    .expect("an explicit ui block parses");

    assert_eq!(ui.resource_uri_template, "ui://acme/{artifact_id}");
    assert_eq!(ui.visibility, vec![ToolVisibility::App]);
}

#[test]
fn tool_metadata_without_a_ui_block_carries_no_ui_defaults() {
    let meta: ToolMetadata =
        serde_yaml::from_str("terminal_on_success: true\n").expect("tool metadata parses");

    assert!(meta.terminal_on_success);
    assert!(
        meta.ui.is_none(),
        "a tool that declares no ui block renders nothing, rather than inheriting a template"
    );
}

#[test]
fn omitted_settings_fall_back_to_the_platform_port_base_and_working_directory() {
    let settings: Settings =
        serde_yaml::from_str("auto_build: true\nbuild_timeout: 60\nhealth_check_timeout: 5\n")
            .expect("settings without the optional fields parse");

    assert_eq!(settings.base_port, 5000);
    assert_eq!(settings.working_dir, "/app");
}

#[test]
fn declared_settings_override_the_port_base_and_working_directory() {
    let settings: Settings = serde_yaml::from_str(
        "auto_build: false\nbuild_timeout: 1\nhealth_check_timeout: 2\nbase_port: \
         6100\nworking_dir: /srv/mcp\n",
    )
    .expect("settings with the optional fields parse");

    assert_eq!(settings.base_port, 6100);
    assert_eq!(settings.working_dir, "/srv/mcp");
}
