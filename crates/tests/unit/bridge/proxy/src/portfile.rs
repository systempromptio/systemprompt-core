use systemprompt_bridge::proxy::{identity, portfile};

fn in_sandbox<T>(temp: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), f)
}

#[test]
fn a_written_port_reads_back() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        portfile::write(48219).expect("record the bound port");
        let record = portfile::read().expect("the record reads back");
        assert_eq!(record.port, 48219);
        assert_eq!(record.pid, std::process::id());
        assert!(record.install_id.same_install(&identity::install_id()));
        assert_eq!(portfile::preferred_port(), Some(48219));
    });
}

#[test]
fn a_missing_file_is_not_an_error() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        assert!(portfile::read().is_none());
        assert!(portfile::preferred_port().is_none());
    });
}

#[test]
fn corrupt_content_is_ignored_rather_than_fatal() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let path = portfile::portfile_path().expect("path");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, b"{ not json").expect("write garbage");
        assert!(
            portfile::read().is_none(),
            "a corrupt record must degrade to the default port, not panic"
        );
    });
}

#[test]
fn a_record_from_another_schema_is_ignored() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let path = portfile::portfile_path().expect("path");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(
            &path,
            br#"{"schema":99,"port":48219,"pid":1,"install_id":"x","config_dir":"/tmp","bound_at_unix":0,"version":"0"}"#,
        )
        .expect("write future record");
        assert!(portfile::read().is_none());
    });
}

#[test]
fn a_record_from_another_install_is_ignored() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let path = portfile::portfile_path().expect("path");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        // Why: a config dir copied between machines carries a port that means
        // nothing here, and following it would land on someone else's proxy.
        std::fs::write(
            &path,
            br#"{"schema":1,"port":48219,"pid":1,"install_id":"someone-else","config_dir":"/tmp","bound_at_unix":0,"version":"0"}"#,
        )
        .expect("write foreign record");
        assert!(portfile::read().is_none());
    });
}

#[test]
fn clear_leaves_a_record_that_is_not_ours() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        portfile::write(48219).expect("record");
        let path = portfile::portfile_path().expect("path");

        // Rewrite with a different pid: a sibling took over after we crashed.
        let raw = std::fs::read_to_string(&path).expect("read");
        let mine = format!("\"pid\": {}", std::process::id());
        std::fs::write(&path, raw.replace(&mine, "\"pid\": 999999")).expect("rewrite pid");

        portfile::clear();
        assert!(
            path.exists(),
            "clearing must not delete a record another process owns"
        );

        // Our own record does go.
        portfile::write(48219).expect("re-record as us");
        portfile::clear();
        assert!(!path.exists(), "our own record is removed on shutdown");
    });
}

#[cfg(unix)]
#[test]
fn the_record_is_not_world_readable() {
    use std::os::unix::fs::PermissionsExt as _;
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        portfile::write(48219).expect("record");
        let path = portfile::portfile_path().expect("path");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    });
}
