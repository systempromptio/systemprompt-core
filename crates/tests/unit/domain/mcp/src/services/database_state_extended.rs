//! Extended tests for `services::database::state` free functions not yet
//! covered in `database_state.rs`.

use std::fs;
use systemprompt_mcp::services::database::state::get_binary_mtime_for_service;
use systemprompt_models::profile::PathsConfig;
use systemprompt_models::AppPaths;

fn paths_with_bin(bin_dir: &str) -> AppPaths {
    let cfg = PathsConfig {
        system: "/tmp".to_owned(),
        services: "/tmp".to_owned(),
        bin: bin_dir.to_owned(),
        web_path: Some("/tmp".to_owned()),
        storage: Some("/tmp".to_owned()),
        geoip_database: None,
    };
    AppPaths::from_profile(&cfg).expect("paths")
}

#[test]
fn get_binary_mtime_for_service_unknown_binary_returns_none() {
    let paths = paths_with_bin("/tmp");
    let result = get_binary_mtime_for_service(&paths, "no-such-binary-xyzzy-never-exists");
    assert!(result.is_none());
}

#[test]
fn get_binary_mtime_for_service_with_created_file_returns_some() {
    let dir = std::env::temp_dir().join(format!("mcp_bin_test_{}", uuid::Uuid::new_v4().simple()));
    fs::create_dir_all(&dir).expect("create tmp dir");
    let svc_name = "test-mcp-service";
    let bin_name = format!("{svc_name}{}", std::env::consts::EXE_SUFFIX);
    fs::write(dir.join(&bin_name), b"fake binary").expect("write");

    let paths = paths_with_bin(dir.to_str().expect("utf8"));
    let result = get_binary_mtime_for_service(&paths, svc_name);
    let _ = fs::remove_dir_all(&dir);

    assert!(result.is_some(), "created binary file should have mtime");
}

#[test]
fn get_binary_mtime_for_service_wrong_dir_returns_none() {
    let paths = paths_with_bin("/nonexistent/bin/dir");
    let result = get_binary_mtime_for_service(&paths, "any-service");
    assert!(result.is_none());
}
