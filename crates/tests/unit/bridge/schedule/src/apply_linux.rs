//! Registering and deregistering the sync timer through the host scheduler,
//! on the platform whose backend is systemd user units.

#![cfg(not(any(target_os = "macos", target_os = "windows")))]

use std::path::{Path, PathBuf};

use systemprompt_bridge::install::{
    ScheduleRemoval, ScheduleStatus, apply_gui_autostart, apply_schedule, gui_autostart_status,
    remove_gui_autostart, remove_schedule, schedule_label, schedule_status,
};
use systemprompt_bridge::schedule::Os;
use systemprompt_bridge::schedule::status::ScheduleStatusCache;

fn sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let home = tempfile::TempDir::new().expect("home tempdir");
    let path = home.path().to_path_buf();
    temp_env::with_vars(
        [
            ("HOME", Some(path.to_string_lossy().into_owned())),
            ("SUDO_USER", None),
        ],
        || f(&path),
    )
}

fn units_dir(home: &Path) -> PathBuf {
    home.join(".config").join("systemd").join("user")
}

fn binary() -> PathBuf {
    PathBuf::from("/usr/local/bin/systemprompt-bridge")
}

#[test]
fn asking_for_another_platforms_schedule_is_refused_before_anything_is_written() {
    sandbox(|home| {
        let cache = ScheduleStatusCache::default();
        let err = apply_schedule(&cache, Os::Windows, &binary())
            .expect_err("a Windows schedule cannot be applied from Linux");
        assert!(
            err.to_string().to_lowercase().contains("os"),
            "the error should name the platform mismatch, got {err}"
        );
        assert!(
            !units_dir(home).exists(),
            "a refused request must not have written any unit"
        );
    });
}

#[test]
fn applying_the_linux_schedule_writes_the_timer_service_and_proxy_units() {
    sandbox(|home| {
        let cache = ScheduleStatusCache::default();
        let applied = apply_schedule(&cache, Os::Linux, &binary()).expect("units are written");

        let unit = schedule_label();
        assert_eq!(applied.label, unit);
        assert_eq!(applied.path, units_dir(home).join(format!("{unit}.timer")));

        let service = units_dir(home).join(format!("{unit}.service"));
        assert!(
            service.is_file(),
            "the service unit is written beside the timer"
        );
        assert!(applied.path.is_file());

        let written = std::fs::read_to_string(&applied.path).expect("read the timer");
        assert!(
            written.contains("[Timer]"),
            "the timer half of the template is what lands in the .timer file, got:\n{written}"
        );
        let service_text = std::fs::read_to_string(&service).expect("read the service");
        assert!(service_text.contains("[Service]"));
        assert!(
            service_text.contains("/usr/local/bin/systemprompt-bridge"),
            "the unit must invoke the binary it was applied for"
        );

        assert!(
            applied.lines.iter().any(|l| l.starts_with("wrote: ")),
            "the applied report lists what it wrote: {:?}",
            applied.lines
        );
    });
}

#[test]
fn a_written_schedule_reports_itself_installed_and_a_fresh_home_does_not() {
    sandbox(|_| {
        assert_eq!(
            schedule_status(&ScheduleStatusCache::default()),
            ScheduleStatus::NotInstalled
        );

        let cache = ScheduleStatusCache::default();
        apply_schedule(&cache, Os::Linux, &binary()).expect("apply");
        assert_eq!(schedule_status(&cache), ScheduleStatus::Installed);

        assert_eq!(
            schedule_status(&ScheduleStatusCache::default()),
            ScheduleStatus::Installed,
            "an uncached probe reads the unit off disk"
        );
    });
}

#[test]
fn removing_a_schedule_that_was_never_applied_reports_nothing_to_remove() {
    sandbox(|_| {
        let cache = ScheduleStatusCache::default();
        let removal = remove_schedule(&cache);
        let ScheduleRemoval::NotInstalled(label) = removal else {
            panic!("expected NotInstalled, got {removal:?}");
        };
        assert_eq!(label, schedule_label());
        assert_eq!(schedule_status(&cache), ScheduleStatus::NotInstalled);
    });
}

#[test]
fn removing_an_applied_schedule_deletes_every_unit_it_wrote() {
    sandbox(|home| {
        let cache = ScheduleStatusCache::default();
        apply_schedule(&cache, Os::Linux, &binary()).expect("apply");

        let removal = remove_schedule(&cache);
        let ScheduleRemoval::Removed(what) = removal else {
            panic!("expected Removed, got {removal:?}");
        };
        assert!(what.contains(schedule_label()), "got {what}");

        let unit = schedule_label();
        assert!(!units_dir(home).join(format!("{unit}.timer")).exists());
        assert!(!units_dir(home).join(format!("{unit}.service")).exists());
        assert_eq!(schedule_status(&cache), ScheduleStatus::NotInstalled);
    });
}

#[test]
fn applying_the_schedule_twice_replaces_the_units_rather_than_adding_a_second_pair() {
    sandbox(|home| {
        let cache = ScheduleStatusCache::default();
        apply_schedule(&cache, Os::Linux, &binary()).expect("first apply");
        let first = std::fs::read_dir(units_dir(home))
            .expect("units dir")
            .count();

        apply_schedule(&cache, Os::Linux, &binary()).expect("second apply");
        let second = std::fs::read_dir(units_dir(home))
            .expect("units dir")
            .count();

        assert_eq!(first, second, "re-applying is idempotent");
    });
}

#[test]
fn this_platform_has_no_desktop_shell_so_gui_autostart_is_refused_rather_than_half_wired() {
    sandbox(|_| {
        let cache = ScheduleStatusCache::default();
        let err = apply_gui_autostart(&cache, &binary())
            .expect_err("there is no GUI on this platform to autostart");
        assert!(err.to_string().to_lowercase().contains("os"), "got {err}");

        assert_eq!(
            gui_autostart_status(&ScheduleStatusCache::default()),
            ScheduleStatus::NotInstalled,
            "not-registered is the whole truth here, not Unknown"
        );

        let removal = remove_gui_autostart(&cache);
        assert!(
            matches!(removal, ScheduleRemoval::NotInstalled(_)),
            "got {removal:?}"
        );
    });
}

#[test]
fn the_schedule_label_is_brand_scoped_so_two_brands_do_not_collide() {
    let label = schedule_label();
    assert!(!label.is_empty());
    assert!(
        !label.contains(' ') && !label.contains('/'),
        "a systemd unit name cannot carry spaces or slashes, got {label}"
    );
}
