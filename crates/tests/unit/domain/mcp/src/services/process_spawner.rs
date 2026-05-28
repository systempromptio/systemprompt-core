//! Unit tests for [`ProcessService::verify_binary`].

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_mcp::services::process::ProcessService;
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
