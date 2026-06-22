//! Tests for `display_service_status` with non-empty data, covering the
//! running/error counting branches and the per-server iteration loop.

use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_mcp::services::monitoring::status::{ServiceStatus, display_service_status};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn make_config(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: "bin".to_owned(),
        enabled: true,
        display_in_web: false,
        port: 0,
        crate_path: PathBuf::from("."),
        display_name: name.to_owned(),
        description: name.to_owned(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.1.0".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn status(state: &str) -> ServiceStatus {
    ServiceStatus {
        state: state.to_owned(),
        pid: None,
        health: state.to_owned(),
        uptime_seconds: None,
        tools_count: 0,
        latency_ms: None,
        auth_required: false,
    }
}

#[test]
fn display_service_status_single_running() {
    let servers = vec![make_config("svc-a")];
    let mut data = HashMap::new();
    data.insert("svc-a".to_owned(), status("running"));
    display_service_status(&servers, &data);
}

#[test]
fn display_service_status_mixed_states() {
    let servers = vec![
        make_config("alpha"),
        make_config("beta"),
        make_config("gamma"),
    ];
    let mut data = HashMap::new();
    data.insert("alpha".to_owned(), status("running"));
    data.insert("beta".to_owned(), status("error"));
    data.insert("gamma".to_owned(), status("stopped"));
    display_service_status(&servers, &data);
}

#[test]
fn display_service_status_all_error() {
    let servers = vec![make_config("err1"), make_config("err2")];
    let mut data = HashMap::new();
    data.insert("err1".to_owned(), status("error"));
    data.insert("err2".to_owned(), status("error"));
    display_service_status(&servers, &data);
}

#[test]
fn display_service_status_server_not_in_data() {
    let servers = vec![make_config("known"), make_config("unknown-svc")];
    let mut data = HashMap::new();
    data.insert("known".to_owned(), status("running"));
    display_service_status(&servers, &data);
}

#[test]
fn display_service_status_empty_servers() {
    let data: HashMap<String, ServiceStatus> = HashMap::new();
    display_service_status(&[], &data);
}

#[test]
fn display_service_status_many_running() {
    let names = ["s1", "s2", "s3", "s4", "s5"];
    let servers: Vec<_> = names.iter().map(|n| make_config(n)).collect();
    let mut data = HashMap::new();
    for n in names {
        data.insert(n.to_owned(), status("running"));
    }
    display_service_status(&servers, &data);
}
