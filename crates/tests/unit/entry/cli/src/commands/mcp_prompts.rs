//! Tests for the `plugins mcp` interactive server-selection prompts, driven
//! through `ScriptedPrompter` without contacting any MCP server.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_cli::plugins::mcp::call::prompt_server_selection as call_prompt_server_selection;
use systemprompt_cli::plugins::mcp::validate::prompt_server_selection as validate_prompt_server_selection;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{Deployment, McpServerType, OAuthRequirement};
use systemprompt_models::services::ServicesConfig;

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn deployment(port: u16) -> Deployment {
    Deployment {
        server_type: McpServerType::Internal,
        binary: "bin".to_owned(),
        package: None,
        port,
        endpoint: None,
        enabled: true,
        display_in_web: true,
        dev_only: false,
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::default(),
        model_config: None,
        env_vars: vec![],
        external_auth: None,
        headers: HashMap::default(),
    }
}

fn services_config(names: &[&str]) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    for (index, name) in names.iter().enumerate() {
        config
            .mcp_servers
            .insert((*name).to_owned(), deployment(5000 + index as u16));
    }
    config
}

#[test]
fn validate_server_selection_returns_choice() {
    let config = services_config(&["gamma", "alpha", "beta"]);
    let prompter = scripted(&["0"]);
    let selected = validate_prompt_server_selection(&prompter, &config).expect("selection made");
    assert_eq!(selected, "alpha");
}

#[test]
fn validate_server_selection_errors_when_empty() {
    let config = ServicesConfig::default();
    let prompter = scripted(&["0"]);
    let err =
        validate_prompt_server_selection(&prompter, &config).expect_err("no servers configured");
    assert!(err.to_string().contains("No MCP servers configured"));
}

#[test]
fn validate_server_selection_rejects_out_of_range_index() {
    let config = services_config(&["alpha"]);
    let prompter = scripted(&["3"]);
    let err = validate_prompt_server_selection(&prompter, &config)
        .expect_err("index beyond available servers");
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn call_server_selection_returns_choice() {
    let config = services_config(&["gamma", "alpha", "beta"]);
    let prompter = scripted(&["2"]);
    let selected = call_prompt_server_selection(&prompter, &config).expect("selection made");
    assert_eq!(selected, "gamma");
}

#[test]
fn call_server_selection_errors_when_empty() {
    let config = ServicesConfig::default();
    let prompter = scripted(&["0"]);
    let err = call_prompt_server_selection(&prompter, &config).expect_err("no servers configured");
    assert!(err.to_string().contains("No MCP servers configured"));
}
