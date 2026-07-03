//! Tests for the pure spawn-environment assembly and log-file helpers in
//! `services::process::spawner`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use systemprompt_mcp::services::process::spawner::{
    SpawnEnvSpec, build_environment, open_server_log, rotate_log_if_needed,
    serialize_server_configs,
};
use systemprompt_models::AppPaths;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn make_config(name: &str, env_vars: Vec<String>) -> McpServerConfig {
    McpServerConfig {
        name: name.to_string(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: "fake-bin".to_string(),
        enabled: true,
        display_in_web: true,
        port: 65001,
        crate_path: PathBuf::from("."),
        display_name: "spawn".to_string(),
        description: "spawn".to_string(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
        },
        tools: HashMap::default(),
        model_config: None,
        env_vars,
        version: "0.0.1".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: HashMap::default(),
    }
}

fn spec<'a>(config: &'a McpServerConfig, root: &'a Path) -> SpawnEnvSpec<'a> {
    SpawnEnvSpec {
        config,
        system_root: root,
        database_type: "postgres",
        profile_path: "/profiles/local",
        tools_config_json: "{}",
        server_model_config_json: "null",
    }
}

fn env_map(pairs: Vec<(String, String)>) -> HashMap<String, String> {
    pairs.into_iter().collect()
}

#[test]
fn build_environment_sets_core_variables() {
    let config = make_config("svc-env", vec![]);
    let root = Path::new("/srv/app");
    let env = env_map(build_environment(&spec(&config, root), &[], |_| None));

    assert_eq!(env.get("SYSTEMPROMPT_PROFILE").unwrap(), "/profiles/local");
    assert_eq!(env.get("SYSTEMPROMPT_SUBPROCESS").unwrap(), "1");
    assert_eq!(env.get("DATABASE_TYPE").unwrap(), "postgres");
    assert_eq!(env.get("MCP_SERVICE_ID").unwrap(), "svc-env");
    assert_eq!(env.get("MCP_PORT").unwrap(), "65001");
    assert_eq!(env.get("MCP_TOOLS_CONFIG").unwrap(), "{}");
    assert_eq!(env.get("MCP_SERVER_MODEL_CONFIG").unwrap(), "null");
    assert_eq!(env.get("SYSTEM_PATH").unwrap(), "/srv/app");
    assert!(!env.contains_key("PATH"));
    assert!(!env.contains_key("HOME"));
}

#[test]
fn build_environment_inherits_path_home_and_trust_allowlist() {
    let config = make_config("svc-inherit", vec![]);
    let inherited: HashMap<&str, &str> = [
        ("PATH", "/usr/bin"),
        ("HOME", "/home/tester"),
        ("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS", "sealed.internal"),
    ]
    .into_iter()
    .collect();

    let env = env_map(build_environment(
        &spec(&config, Path::new("/srv")),
        &[],
        |name| inherited.get(name).map(|v| (*v).to_owned()),
    ));

    assert_eq!(env.get("PATH").unwrap(), "/usr/bin");
    assert_eq!(env.get("HOME").unwrap(), "/home/tester");
    assert_eq!(
        env.get("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS").unwrap(),
        "sealed.internal"
    );
}

#[test]
fn build_environment_includes_secrets_and_configured_vars() {
    let config = make_config(
        "svc-secrets",
        vec!["MY_API_KEY".to_owned(), "MISSING_VAR".to_owned()],
    );
    let secrets = vec![("JWT_SECRET".to_owned(), "abc".to_owned())];
    let lookup_env: HashMap<&str, &str> = [("MY_API_KEY", "key-value")].into_iter().collect();

    let env = env_map(build_environment(
        &spec(&config, Path::new("/srv")),
        &secrets,
        |name| lookup_env.get(name).map(|v| (*v).to_owned()),
    ));

    assert_eq!(env.get("JWT_SECRET").unwrap(), "abc");
    assert_eq!(env.get("MY_API_KEY").unwrap(), "key-value");
    assert!(!env.contains_key("MISSING_VAR"));
}

#[test]
fn serialize_server_configs_round_trips() {
    let config = make_config("svc-json", vec![]);
    let (tools, model) = serialize_server_configs(&config).expect("serializes");
    assert_eq!(tools, "{}");
    assert_eq!(model, "null");
}

#[test]
fn rotate_log_if_needed_renames_oversized_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("mcp-big.log");
    let big = vec![b'x'; 10 * 1024 * 1024 + 1];
    std::fs::write(&log, &big).expect("write big log");

    rotate_log_if_needed(&log);

    assert!(!log.exists(), "oversized log should be rotated away");
    assert!(dir.path().join("mcp-big.log.old").exists());
}

#[test]
fn rotate_log_if_needed_keeps_small_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("mcp-small.log");
    std::fs::write(&log, b"tiny").expect("write log");

    rotate_log_if_needed(&log);

    assert!(log.exists());
    assert!(!dir.path().join("mcp-small.log.old").exists());
}

#[test]
fn open_server_log_creates_log_directory_and_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths_config = PathsConfig {
        system: dir.path().display().to_string(),
        services: dir.path().display().to_string(),
        bin: dir.path().display().to_string(),
        web_path: Some(dir.path().display().to_string()),
        storage: Some(dir.path().display().to_string()),
        geoip_database: None,
    };
    let paths = Arc::new(AppPaths::from_profile(&paths_config).expect("paths"));
    let config = make_config("logtest", vec![]);

    open_server_log(&paths, &config).expect("log file should open");
}
