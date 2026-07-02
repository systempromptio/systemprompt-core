//! Exercises the process spawner against the bootstrap profile: detached
//! spawn of a stub binary, binary verification, log handling, and the debug
//! build path via a stubbed `cargo` on `PATH`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use systemprompt_mcp::services::process::spawner::{
    build_server, open_server_log, serialize_server_configs, spawn_server, verify_binary,
};
use systemprompt_models::AppPaths;
use systemprompt_test_fixtures::{TestBootstrap, ensure_test_bootstrap};

use crate::harness::internal_mcp_config;

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("mark executable");
}

fn app_paths(bootstrap: &TestBootstrap) -> AppPaths {
    bootstrap.app_paths.clone()
}

fn stub_cargo_on_path(exit_code: u8) {
    let dir = tempfile::tempdir().expect("cargo stub dir");
    let script = format!("#!/bin/sh\necho stub-stderr >&2\nexit {exit_code}\n");
    write_executable(&dir.path().join("cargo"), &script);
    let path = std::env::var("PATH").unwrap_or_default();
    // SAFETY: nextest runs each test in its own process, so mutating PATH is
    // process-local to this test.
    unsafe {
        std::env::set_var("PATH", format!("{}:{path}", dir.path().display()));
    }
    std::mem::forget(dir);
}

#[test]
fn spawn_server_launches_detached_stub() {
    let bootstrap = ensure_test_bootstrap();
    let name = format!("spawnok{}", std::process::id());
    write_executable(
        &bootstrap.bin_path.join(format!("{name}-bin")),
        "#!/bin/sh\nexit 0\n",
    );

    let config = internal_mcp_config(&name, 59321);
    let paths = app_paths(bootstrap);

    let pid = spawn_server(&paths, &config).expect("stub spawns");
    assert!(pid > 0);

    let log_path = paths.system().logs().join(format!("mcp-{name}.log"));
    assert!(log_path.exists());
}

#[test]
fn spawn_server_missing_binary_errors() {
    let bootstrap = ensure_test_bootstrap();
    let config = internal_mcp_config("spawn-missing", 59322);
    let err = spawn_server(&app_paths(bootstrap), &config).expect_err("missing binary");
    assert!(err.to_string().contains("Failed to find binary"));
}

#[test]
fn verify_binary_checks_presence() {
    let bootstrap = ensure_test_bootstrap();
    let name = format!("verifyok{}", std::process::id());
    write_executable(
        &bootstrap.bin_path.join(format!("{name}-bin")),
        "#!/bin/sh\nexit 0\n",
    );
    let paths = app_paths(bootstrap);

    verify_binary(&paths, &internal_mcp_config(&name, 0)).expect("binary verifies");
    assert!(verify_binary(&paths, &internal_mcp_config("verify-missing", 0)).is_err());
}

#[test]
fn serialize_server_configs_round_trips() {
    let mut config = internal_mcp_config("serialize", 0);
    config.tools.insert(
        "echo".to_owned(),
        serde_json::from_value(serde_json::json!({"terminal_on_success": true}))
            .expect("tool metadata"),
    );

    let (tools_json, model_json) = serialize_server_configs(&config).expect("serializes");
    assert!(tools_json.contains("terminal_on_success"));
    assert_eq!(model_json, "null");
}

#[test]
fn open_server_log_rotates_oversized_file() {
    let bootstrap = ensure_test_bootstrap();
    let paths = app_paths(bootstrap);
    let name = format!("rotate{}", std::process::id());
    let config = internal_mcp_config(&name, 0);

    let log_dir = paths.system().logs();
    std::fs::create_dir_all(&log_dir).expect("logs dir");
    let log_path = log_dir.join(format!("mcp-{name}.log"));
    std::fs::write(&log_path, vec![b'x'; 11 * 1024 * 1024]).expect("oversized log");

    let _file = open_server_log(&paths, &config).expect("log opens");
    assert!(log_path.with_extension("log.old").exists());
}

#[test]
fn build_server_succeeds_with_stub_cargo() {
    stub_cargo_on_path(0);
    build_server(&internal_mcp_config("buildok", 0)).expect("stub build succeeds");
}

#[test]
fn build_server_surfaces_stub_failure() {
    stub_cargo_on_path(1);
    let err = build_server(&internal_mcp_config("buildfail", 0)).expect_err("stub build fails");
    assert!(err.to_string().contains("Build failed"));
}
