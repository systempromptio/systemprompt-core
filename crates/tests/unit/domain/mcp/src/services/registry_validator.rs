//! Pre-flight validation of the MCP server registry. `validate_registry` is
//! the only public entry point and nothing calls it from a test, so none of its
//! four check suites — port conflicts, per-server fields, OAuth coherence, and
//! internal-vs-external constraints — has ever run.

use std::path::PathBuf;

use systemprompt_mcp::services::registry::validator::validate_registry;
use systemprompt_models::auth::{JwtAudience, Permission};
use systemprompt_models::mcp::RegistryConfig;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn internal_server(name: &str, port: u16) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: format!("{name}-bin"),
        enabled: true,
        display_in_web: true,
        port,
        // The crate path must exist or the field checks short-circuit before
        // display_name/description are ever looked at.
        crate_path: PathBuf::from("."),
        display_name: format!("{name} Server"),
        description: name.to_owned(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: Default::default(),
        model_config: None,
        env_vars: vec![],
        version: "0.0.1".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

fn external_server(name: &str, endpoint: &str) -> McpServerConfig {
    let mut server = internal_server(name, 0);
    server.server_type = McpServerType::External;
    server.binary = String::new();
    server.remote_endpoint = endpoint.to_owned();
    server
}

fn registry(servers: Vec<McpServerConfig>) -> RegistryConfig {
    RegistryConfig {
        servers,
        registry_url: None,
        cache_dir: None,
    }
}

#[test]
fn a_registry_of_well_formed_servers_passes_every_check() {
    let config = registry(vec![
        internal_server("alpha", 5101),
        internal_server("beta", 5102),
        external_server("gamma", "https://example.invalid/mcp"),
    ]);

    validate_registry(&config).expect("a coherent registry validates");
}

#[test]
fn an_empty_registry_is_valid() {
    validate_registry(&registry(vec![])).expect("nothing to validate");
}

#[test]
fn two_enabled_internal_servers_on_one_port_are_rejected() {
    let config = registry(vec![
        internal_server("alpha", 5101),
        internal_server("beta", 5101),
    ]);

    let err = validate_registry(&config).expect_err("two servers cannot share a port");
    let message = err.to_string();
    assert!(message.contains("Port conflicts"), "got: {message}");
    assert!(
        message.contains("beta:5101"),
        "the failure names the losing server and its port: {message}"
    );
}

#[test]
fn a_disabled_server_may_reuse_an_enabled_servers_port() {
    let mut disabled = internal_server("beta", 5101);
    disabled.enabled = false;

    validate_registry(&registry(vec![internal_server("alpha", 5101), disabled]))
        .expect("a disabled server binds nothing, so it cannot conflict");
}

#[test]
fn external_servers_do_not_contend_for_ports() {
    let config = registry(vec![
        external_server("one", "https://a.invalid/mcp"),
        external_server("two", "https://b.invalid/mcp"),
    ]);

    validate_registry(&config).expect("external servers hold no local port");
}

#[test]
fn an_internal_server_on_a_privileged_port_is_rejected() {
    let config = registry(vec![internal_server("alpha", 80)]);

    let err = validate_registry(&config).expect_err("ports below 1024 need privilege");
    assert!(err.to_string().contains("invalid port 80"), "got: {err}");
}

#[test]
fn an_internal_server_whose_crate_path_is_missing_is_rejected() {
    let mut server = internal_server("alpha", 5101);
    server.crate_path = PathBuf::from("/nonexistent/crate/path/for/tests");

    let err = validate_registry(&registry(vec![server])).expect_err("the crate must be on disk");
    assert!(
        err.to_string().contains("crate path does not exist"),
        "got: {err}"
    );
}

#[test]
fn a_server_missing_its_display_name_or_description_is_rejected() {
    let mut server = internal_server("alpha", 5101);
    server.display_name = String::new();
    server.description = String::new();

    let err = validate_registry(&registry(vec![server])).expect_err("both fields are required");
    let message = err.to_string();
    assert!(message.contains("missing display_name"), "got: {message}");
    assert!(
        message.contains("missing description"),
        "every failure is reported together rather than the first only: {message}"
    );
}

#[test]
fn oauth_without_scopes_is_rejected_as_incoherent() {
    let mut server = internal_server("alpha", 5101);
    server.oauth.required = true;

    let err = validate_registry(&registry(vec![server]))
        .expect_err("requiring oauth with no scopes authorises nothing");
    assert!(err.to_string().contains("no scopes defined"), "got: {err}");
}

#[test]
fn oauth_with_scopes_is_accepted() {
    let mut server = internal_server("alpha", 5101);
    server.oauth.required = true;
    server.oauth.scopes = vec![Permission::Mcp];

    validate_registry(&registry(vec![server])).expect("a scoped oauth requirement is coherent");
}

#[test]
fn an_internal_server_without_a_binary_is_rejected() {
    let mut server = internal_server("alpha", 5101);
    server.binary = String::new();

    let err = validate_registry(&registry(vec![server]))
        .expect_err("an internal server must be runnable");
    assert!(err.to_string().contains("no binary"), "got: {err}");
}

#[test]
fn an_external_server_without_an_endpoint_is_rejected() {
    let server = external_server("gamma", "");

    let err = validate_registry(&registry(vec![server]))
        .expect_err("an external server with no endpoint is unreachable");
    assert!(err.to_string().contains("no remote endpoint"), "got: {err}");
}

#[test]
fn an_external_server_carrying_a_binary_is_rejected() {
    let mut server = external_server("gamma", "https://example.invalid/mcp");
    server.binary = "gamma-bin".to_owned();

    let err = validate_registry(&registry(vec![server]))
        .expect_err("an external server has no local process to run");
    assert!(
        err.to_string().contains("should not have a binary"),
        "got: {err}"
    );
}

#[test]
fn disabled_servers_are_exempt_from_every_field_check() {
    let mut broken = internal_server("broken", 80);
    broken.enabled = false;
    broken.binary = String::new();
    broken.display_name = String::new();
    broken.description = String::new();
    broken.crate_path = PathBuf::from("/nonexistent");

    validate_registry(&registry(vec![broken]))
        .expect("a disabled server is never brought up, so it is never validated");
}
