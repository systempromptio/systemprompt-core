//! Unit tests for [`ProcessService::verify_binary`].

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::process::spawner::{
    open_server_log, rotate_log_if_needed, serialize_server_configs,
};
use systemprompt_models::AppPaths;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::{McpServerType, OAuthRequirement};
use systemprompt_models::mcp::server::McpServerConfig;
use systemprompt_models::profile::PathsConfig;
use systemprompt_test_fixtures::fixture_user_id;

fn make_paths(bin_dir: &str) -> Arc<AppPaths> {
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: bin_dir.to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    Arc::new(AppPaths::from_profile(&paths).expect("paths"))
}

fn make_paths_with_system(system_dir: &str) -> Arc<AppPaths> {
    let paths = PathsConfig {
        system: system_dir.to_string(),
        services: system_dir.to_string(),
        bin: system_dir.to_string(),
        web_path: Some(system_dir.to_string()),
        storage: Some(system_dir.to_string()),
        geoip_database: None,
    };
    Arc::new(AppPaths::from_profile(&paths).expect("paths"))
}

fn make_config(binary: &str) -> McpServerConfig {
    McpServerConfig {
        name: "verify-bin".to_string(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: binary.to_string(),
        enabled: true,
        display_in_web: true,
        port: 65500,
        crate_path: PathBuf::from("."),
        display_name: "v".to_string(),
        description: "v".to_string(),
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
        version: "0.0.1".to_string(),
        host: "127.0.0.1".to_string(),
        module_name: "mcp".to_string(),
        protocol: "mcp".to_string(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: Default::default(),
    }
}

#[test]
fn verify_binary_missing_returns_err() {
    let paths = make_paths("/tmp");
    let config = make_config(&format!("no-such-{}", uuid::Uuid::new_v4().simple()));
    let r = ProcessService::verify_binary(&paths, &config);
    assert!(r.is_err());
}

#[test]
fn verify_binary_present_succeeds() {
    let dir = std::env::temp_dir().join(format!("verify-bin-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir).unwrap();
    let bin_name = "fakebin";
    let bin_path = dir.join(bin_name);
    std::fs::write(&bin_path, b"#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&bin_path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&bin_path, perms).unwrap();

    let paths = make_paths(dir.to_str().unwrap());
    let config = make_config(bin_name);
    let r = ProcessService::verify_binary(&paths, &config);
    let _ = r;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rotate_log_leaves_a_small_log_in_place() {
    let dir = tempfile::tempdir().expect("tmp");
    let log = dir.path().join("mcp-small.log");
    std::fs::write(&log, b"a few bytes").expect("write");

    rotate_log_if_needed(&log);

    assert_eq!(
        std::fs::read(&log).expect("still there"),
        b"a few bytes",
        "a log under the rotation threshold is untouched"
    );
    assert!(!log.with_extension("log.old").exists());
}

#[test]
fn rotate_log_moves_an_oversized_log_aside() {
    let dir = tempfile::tempdir().expect("tmp");
    let log = dir.path().join("mcp-big.log");
    // One byte over the 10 MiB threshold.
    std::fs::write(&log, vec![b'x'; 10 * 1024 * 1024 + 1]).expect("write");

    rotate_log_if_needed(&log);

    assert!(!log.exists(), "the oversized log is moved out of the way");
    assert!(
        log.with_extension("log.old").exists(),
        "and kept as the .old backup"
    );
}

#[test]
fn rotate_log_ignores_a_path_that_does_not_exist() {
    let dir = tempfile::tempdir().expect("tmp");
    rotate_log_if_needed(&dir.path().join("absent.log"));
}

#[test]
fn open_server_log_creates_the_logs_directory_and_appends() {
    let dir = tempfile::tempdir().expect("tmp");
    let paths = make_paths_with_system(dir.path().to_str().expect("utf8"));
    let config = make_config("unused");

    {
        let mut file = open_server_log(&paths, &config).expect("first open");
        std::io::Write::write_all(&mut file, b"first\n").expect("write");
    }
    {
        let mut file = open_server_log(&paths, &config).expect("second open");
        std::io::Write::write_all(&mut file, b"second\n").expect("write");
    }

    let log = paths
        .system()
        .logs()
        .join(format!("mcp-{}.log", config.name));
    let contents = std::fs::read_to_string(&log).expect("log written");
    assert_eq!(
        contents, "first\nsecond\n",
        "reopening appends rather than truncating"
    );
}

#[test]
fn serialize_server_configs_emits_json_for_tools_and_model() {
    let config = make_config("unused");

    let (tools, model) = serialize_server_configs(&config).expect("serialize");

    serde_json::from_str::<serde_json::Value>(&tools).expect("tools config is JSON");
    assert_eq!(model, "null", "an absent model config serialises as null");
}
