//! Unit tests for [`ProcessService::verify_binary`].

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_config::paths::AppPaths;
use systemprompt_mcp::services::process::ProcessService;
use systemprompt_mcp::services::process::spawner::{
    open_server_log, rotate_log_if_needed, serialize_server_configs,
};
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
    Arc::new(
        AppPaths::from_profile(
            &paths,
            systemprompt_models::PathResolution::Canonicalize,
            None,
        )
        .expect("paths"),
    )
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
    Arc::new(
        AppPaths::from_profile(
            &paths,
            systemprompt_models::PathResolution::Canonicalize,
            None,
        )
        .expect("paths"),
    )
}

fn make_config(binary: &str) -> McpServerConfig {
    McpServerConfig {
        name: "verify-bin".to_string(),
        owner: fixture_user_id(),
        server_type: McpServerType::Internal,
        binary: Some(binary.to_string()),
        enabled: true,
        display_in_web: true,
        port: Some(65500),
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
            ema: false,
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

#[test]
fn coverage_log_directory_failure_names_the_unwritable_path() {
    let dir = tempfile::tempdir().unwrap();
    let paths = make_paths_with_system(dir.path().to_str().unwrap());
    let logs = paths.system().logs();
    std::fs::create_dir_all(logs.parent().unwrap()).unwrap();
    std::fs::write(&logs, b"a file blocks directory creation").unwrap();
    let err = open_server_log(&paths, &make_config("unused")).unwrap_err();
    assert!(err.to_string().contains("Failed to create logs directory"));
    assert!(err.to_string().contains(logs.to_str().unwrap()));
}

#[test]
fn coverage_log_open_failure_does_not_remove_an_existing_directory() {
    let dir = tempfile::tempdir().unwrap();
    let paths = make_paths_with_system(dir.path().to_str().unwrap());
    let config = make_config("unused");
    let log = paths
        .system()
        .logs()
        .join(format!("mcp-{}.log", config.name));
    std::fs::create_dir_all(&log).unwrap();
    let err = open_server_log(&paths, &config).unwrap_err();
    assert!(err.to_string().contains("Failed to create log file"));
    assert!(log.is_dir());
}

#[test]
fn coverage_rotation_failure_keeps_the_original_log_readable() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("blocked.log");
    let file = std::fs::File::create(&log).unwrap();
    file.set_len(10 * 1024 * 1024 + 1).unwrap();
    std::fs::create_dir(log.with_extension("log.old")).unwrap();
    rotate_log_if_needed(&log);
    assert_eq!(std::fs::metadata(&log).unwrap().len(), 10 * 1024 * 1024 + 1);
}

#[test]
fn coverage_spawn_invalid_executable_returns_a_detached_start_error() {
    use std::os::unix::fs::PermissionsExt;
    use systemprompt_mcp::services::process::spawner::spawn_server;
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let config = make_config("invalid-executable");
    let binary = boot.bin_path.join(config.binary.as_deref().unwrap());
    std::fs::write(&binary, b"#!/no-such-interpreter-for-coverage\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let err = spawn_server(&boot.app_paths, &config).unwrap_err();
    assert!(
        err.to_string().contains("Failed to start detached"),
        "{err}"
    );
}

#[tokio::test]
async fn spawned_server_receives_service_environment_and_verified_termination_stops_it() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;
    use systemprompt_mcp::services::process::ProcessService;
    use systemprompt_mcp::services::process::utils::{kill_process, process_exists};

    struct OwnedPid {
        pid: Option<u32>,
        service_name: String,
    }
    impl Drop for OwnedPid {
        fn drop(&mut self) {
            if let Some(pid) = self.pid
                && process_exists(pid)
                && systemprompt_loader::subprocess::live_pid_is_subprocess(
                    pid,
                    systemprompt_models::subprocess::MCP_SERVICE_ID_ENV,
                    &self.service_name,
                )
            {
                let _ = kill_process(pid);
            }
        }
    }

    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let binary_name = format!("mcp-spawn-fixture-{unique}");
    let marker = std::env::temp_dir().join(format!("mcp-spawn-marker-{unique}"));
    let binary = boot.bin_path.join(&binary_name);
    std::fs::write(
        &binary,
        format!(
            "#!/bin/sh\nprintf '%s|%s|%s' \"$SYSTEMPROMPT_SUBPROCESS\" \"$MCP_SERVICE_ID\" \"$MCP_PORT\" > '{}'\nexec /bin/sleep 60\n",
            marker.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut config = make_config(&binary_name);
    config.name = format!("spawn-{unique}");
    config.port = Some(65431);

    let pid = ProcessService::spawn_server(&boot.app_paths, &config).expect("spawn fixture");
    let mut cleanup = OwnedPid {
        pid: Some(pid),
        service_name: config.name.clone(),
    };
    assert!(process_exists(pid));
    for _ in 0..80 {
        if marker.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        std::fs::read_to_string(&marker).expect("child environment marker"),
        format!("1|{}|65431", config.name)
    );

    ProcessService::terminate_gracefully_verified(pid, &config.name)
        .await
        .expect("verified termination");
    for _ in 0..80 {
        if !ProcessService::is_running(pid) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(
        !ProcessService::is_running(pid),
        "verified graceful termination must leave no live owned fixture child"
    );
    cleanup.pid = None;
    std::fs::remove_file(binary).ok();
    std::fs::remove_file(marker).ok();
}

#[cfg(unix)]
fn with_cargo_shim(exit_code: i32, test: impl FnOnce(&std::path::Path)) {
    use std::os::unix::fs::PermissionsExt;
    struct PathGuard(Option<std::ffi::OsString>);
    impl Drop for PathGuard {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => unsafe { std::env::set_var("PATH", value) },
                None => unsafe { std::env::remove_var("PATH") },
            }
        }
    }
    let dir = tempfile::tempdir().expect("private cargo shim directory");
    let invocation = dir.path().join("invocation");
    let cargo = dir.path().join("cargo");
    std::fs::write(
        &cargo,
        format!(
            "#!/bin/sh\nprintf '%s' \"$*\" > '{}'\nexit {exit_code}\n",
            invocation.display()
        ),
    )
    .expect("write cargo shim");
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o700))
        .expect("make cargo shim executable");
    let _guard = PathGuard(std::env::var_os("PATH"));
    unsafe { std::env::set_var("PATH", dir.path()) };
    test(&invocation);
}

#[cfg(unix)]
#[test]
fn build_server_invokes_cargo_for_the_exact_configured_package_and_binary() {
    use systemprompt_mcp::services::process::spawner::build_server;
    let config = make_config("owned-mcp-fixture");
    with_cargo_shim(0, |invocation| {
        build_server(&config).expect("successful cargo build exit is accepted");
        assert_eq!(
            std::fs::read_to_string(invocation).expect("captured cargo invocation"),
            "build --package owned-mcp-fixture --bin owned-mcp-fixture"
        );
    });
}

#[cfg(unix)]
#[test]
fn build_server_surfaces_failed_cargo_exit_without_claiming_success() {
    use systemprompt_mcp::services::process::spawner::build_server;
    let config = make_config("failing-mcp-fixture");
    with_cargo_shim(23, |invocation| {
        let error = build_server(&config).expect_err("failed cargo exit must reject the build");
        assert!(
            error
                .to_string()
                .contains("Build failed for verify-bin (binary: failing-mcp-fixture)"),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(invocation).expect("captured cargo invocation"),
            "build --package failing-mcp-fixture --bin failing-mcp-fixture"
        );
    });
}
