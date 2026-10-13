//! Load-time validation of a connector's machine grant: each grant's
//! required and forbidden fields.

use systemprompt_models::mcp::deployment::{ConnectorConfig, ConnectorGrant};

fn parse(yaml: &str) -> ConnectorConfig {
    serde_yaml::from_str(yaml).expect("connector yaml parses")
}

#[test]
fn the_grant_defaults_to_authorization_code() {
    let c = parse("client_id_secret: id\nclient_secret: secret\n");
    assert_eq!(c.grant, ConnectorGrant::AuthorizationCode);
    c.validate("acme").expect("consent connector validates");
}

#[test]
fn an_unknown_grant_field_is_refused() {
    let err = serde_yaml::from_str::<ConnectorConfig>("grant: client_credentials\ntoken: x\n");
    assert!(err.is_err(), "deny_unknown_fields still applies");
}

#[test]
fn client_credentials_validates_with_url_and_secrets() {
    let c = parse(
        "grant: client_credentials\ntoken_url: https://idp.example.com/token\n\
         client_id_secret: id\nclient_secret: secret\n",
    );
    c.validate("jira")
        .expect("complete client_credentials validates");
}

#[test]
fn client_credentials_requires_a_token_url() {
    let c = parse("grant: client_credentials\nclient_id_secret: id\nclient_secret: secret\n");
    let msg = c.validate("jira").unwrap_err().to_string();
    assert!(msg.contains("token_url") && msg.contains("jira"), "{msg}");
}

#[test]
fn client_credentials_requires_the_client_secret() {
    let c = parse(
        "grant: client_credentials\ntoken_url: https://idp.example.com/token\n\
         client_id_secret: id\n",
    );
    let msg = c.validate("jira").unwrap_err().to_string();
    assert!(msg.contains("client_secret"), "{msg}");
}

#[test]
fn a_machine_grant_refuses_a_cleartext_token_url() {
    let c = parse(
        "grant: client_credentials\ntoken_url: http://idp.example.com/token\n\
         client_id_secret: id\nclient_secret: secret\n",
    );
    let msg = c.validate("jira").unwrap_err().to_string();
    assert!(msg.contains("https"), "{msg}");
}

#[test]
fn a_machine_grant_refuses_consent_parameters() {
    let c = parse(
        "grant: client_credentials\ntoken_url: https://idp.example.com/token\n\
         client_id_secret: id\nclient_secret: secret\nauthorization_params:\n  prompt: consent\n",
    );
    let msg = c.validate("jira").unwrap_err().to_string();
    assert!(msg.contains("authorization_params"), "{msg}");
}

#[test]
fn app_installation_validates_with_app_fields() {
    let c = parse(
        "grant: app_installation\n\
         token_url: https://api.github.com/app/installations/{installation_id}/access_tokens\n\
         private_key_secret: gh_app_key\napp_id: \"12345\"\ninstallation_id: \"678\"\n",
    );
    c.validate("github")
        .expect("complete app_installation validates");
}

#[test]
fn app_installation_requires_the_installation_id() {
    let c = parse(
        "grant: app_installation\ntoken_url: https://api.github.com/x\n\
         private_key_secret: gh_app_key\napp_id: \"12345\"\n",
    );
    let msg = c.validate("github").unwrap_err().to_string();
    assert!(msg.contains("installation_id"), "{msg}");
}

#[test]
fn authorization_code_refuses_machine_fields() {
    let c = parse("token_url: https://idp.example.com/token\n");
    let msg = c.validate("acme").unwrap_err().to_string();
    assert!(msg.contains("token_url"), "{msg}");
}
