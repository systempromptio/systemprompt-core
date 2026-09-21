use std::os::unix::fs::PermissionsExt;

use systemprompt_bridge::cli::doctor::Status;
use systemprompt_bridge::cli::doctor::proxy::check_proxy_service;

#[test]
fn proxy_service_diagnostic_tracks_absent_unavailable_active_and_failed_unit_states() {
    let sandbox = tempfile::tempdir().unwrap();
    let home = sandbox.path().join("home");
    let bin = sandbox.path().join("bin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&bin).unwrap();

    temp_env::with_vars(
        [
            ("HOME", Some(home.as_os_str())),
            ("PATH", Some(bin.as_os_str())),
        ],
        || {
            let absent = check_proxy_service().expect("Linux exposes service diagnostics");
            assert_eq!(absent.status, Status::Warn);
            assert!(absent.detail.contains("not present"));

            let unit_dir = home.join(".config/systemd/user");
            std::fs::create_dir_all(&unit_dir).unwrap();
            let unit = format!(
                "{}.service",
                systemprompt_bridge::schedule::proxy_unit_name()
            );
            std::fs::write(
                unit_dir.join(&unit),
                "[Service]\nExecStart=systemprompt-bridge proxy\n",
            )
            .unwrap();

            let unavailable = check_proxy_service().unwrap();
            assert_eq!(unavailable.status, Status::Warn);
            assert!(unavailable.detail.contains("systemctl is unavailable"));

            let systemctl = bin.join("systemctl");
            write_systemctl(&systemctl, "active", 0);
            let active = check_proxy_service().unwrap();
            assert_eq!(active.status, Status::Ok);
            assert_eq!(active.detail, format!("{unit} active"));

            write_systemctl(&systemctl, "failed", 3);
            let failed = check_proxy_service().unwrap();
            assert_eq!(failed.status, Status::Warn);
            assert!(failed.detail.contains("is 'failed'"));
            assert!(failed.detail.contains("enable --now"));
        },
    );
}

fn write_systemctl(path: &std::path::Path, state: &str, exit: i32) {
    std::fs::write(
        path,
        format!("#!/bin/sh\nprintf '%s\\n' '{state}'\nexit {exit}\n"),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions).unwrap();
}
