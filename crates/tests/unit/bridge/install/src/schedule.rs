use systemprompt_bridge::install::{
    InstallError, ScheduleRemoval, apply_schedule, emit_schedule, remove_schedule,
};
use systemprompt_bridge::schedule::Os;
use tempfile::TempDir;

fn home_sandbox<R>(f: impl FnOnce(&std::path::Path) -> R) -> R {
    let home = TempDir::new().expect("home tempdir");
    let path = home.path().to_path_buf();
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(path.display().to_string())),
        ("SUDO_USER", None),
    ];
    let out = temp_env::with_vars(vars, || f(&path));
    drop(home);
    out
}

fn units_dir(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".config").join("systemd").join("user")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn apply_schedule_writes_both_systemd_units() {
    home_sandbox(|home| {
        let binary = std::path::Path::new("/usr/local/bin/systemprompt-bridge");
        let outcome = apply_schedule(Os::Linux, binary);

        let dir = units_dir(home);
        let service = dir.join("systemprompt-bridge-sync.service");
        let timer = dir.join("systemprompt-bridge-sync.timer");
        assert!(service.is_file(), "the .service unit is written");
        assert!(timer.is_file(), "the .timer unit is written");
        assert!(
            std::fs::read_to_string(&service)
                .expect("service body")
                .contains("/usr/local/bin/systemprompt-bridge"),
            "the unit invokes the installed binary"
        );

        match outcome {
            Ok(applied) => {
                assert_eq!(applied.path, timer);
                assert_eq!(applied.label, "systemprompt-bridge-sync");
            },
            Err(InstallError::ScheduleApply(msg)) => assert!(
                msg.contains("systemctl"),
                "a scheduler failure must name systemctl: {msg}"
            ),
            Err(other) => panic!("unexpected error: {other}"),
        }
    });
}

#[test]
fn apply_schedule_refuses_a_foreign_target_os() {
    let foreign = if cfg!(target_os = "windows") {
        Os::Linux
    } else {
        Os::Windows
    };
    let err = apply_schedule(foreign, std::path::Path::new("/bin/true"))
        .expect_err("a foreign OS target is rejected");
    assert!(
        matches!(err, InstallError::ScheduleOsMismatch),
        "expected ScheduleOsMismatch, got {err}"
    );
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn remove_schedule_reports_not_installed_then_removed() {
    home_sandbox(|home| {
        match remove_schedule() {
            ScheduleRemoval::NotInstalled(unit) => {
                assert_eq!(unit, "systemprompt-bridge-sync");
            },
            other => panic!("expected NotInstalled on a clean home, got {other:?}"),
        }

        let dir = units_dir(home);
        std::fs::create_dir_all(&dir).expect("units dir");
        std::fs::write(dir.join("systemprompt-bridge-sync.timer"), "[Timer]\n").expect("timer");
        std::fs::write(dir.join("systemprompt-bridge-sync.service"), "[Service]\n")
            .expect("service");

        match remove_schedule() {
            ScheduleRemoval::Removed(unit) => assert_eq!(unit, "systemprompt-bridge-sync"),
            other => panic!("expected Removed, got {other:?}"),
        }
        assert!(
            !dir.join("systemprompt-bridge-sync.timer").exists()
                && !dir.join("systemprompt-bridge-sync.service").exists(),
            "both units are deleted"
        );
    });
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn remove_schedule_tolerates_a_timer_without_its_service() {
    home_sandbox(|home| {
        let dir = units_dir(home);
        std::fs::create_dir_all(&dir).expect("units dir");
        std::fs::write(dir.join("systemprompt-bridge-sync.timer"), "[Timer]\n").expect("timer");
        match remove_schedule() {
            ScheduleRemoval::Removed(unit) => assert_eq!(unit, "systemprompt-bridge-sync"),
            other => panic!("expected Removed, got {other:?}"),
        }
    });
}

#[test]
fn emit_schedule_writes_the_template_into_the_working_directory() {
    let dir = TempDir::new().expect("cwd tempdir");
    let previous = std::env::current_dir().expect("cwd");
    let vars: Vec<(&str, Option<String>)> = vec![("SUDO_USER", None)];
    temp_env::with_vars(vars, || {
        std::env::set_current_dir(dir.path()).expect("chdir");
        for os in [Os::Linux, Os::Mac, Os::Windows] {
            let emitted = emit_schedule(os, std::path::Path::new("/opt/bridge/bin"))
                .expect("template emitted");
            let body = std::fs::read_to_string(&emitted.path).expect("template body");
            assert!(
                body.contains("/opt/bridge/bin"),
                "the {os:?} template embeds the binary path"
            );
            assert!(
                !emitted.install_hint.is_empty(),
                "each OS carries an install hint"
            );
        }
        std::env::set_current_dir(&previous).expect("restore cwd");
    });
}
