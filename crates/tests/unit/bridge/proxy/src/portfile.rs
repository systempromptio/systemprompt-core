use systemprompt_bridge::proxy::identity::InstallId;
use systemprompt_bridge::proxy::{DEFAULT_PROXY_PORT, MAX_CANDIDATE_PORT, portfile};

fn in_sandbox<T>(temp: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), f)
}

fn establish() -> InstallId {
    InstallId::establish().expect("the sandbox mints an install id")
}

fn seed_raw(body: &[u8]) -> std::path::PathBuf {
    let path = portfile::portfile_path().expect("path");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, body).expect("write record");
    path
}

#[test]
fn a_written_port_reads_back() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        portfile::write(48219, &ours).expect("record the bound port");
        let record = portfile::read(&ours)
            .expect("the record is readable")
            .expect("the record is ours");
        assert_eq!(record.port, 48219);
        assert_eq!(record.pid, std::process::id());
        assert!(record.install_id.same_install(&ours));
        assert_eq!(
            portfile::preferred_port(&ours).expect("readable"),
            Some(48219)
        );
    });
}

#[test]
fn a_missing_file_is_not_an_error() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        assert!(
            portfile::read(&ours)
                .expect("absence is not an error")
                .is_none()
        );
        assert!(
            portfile::preferred_port(&ours)
                .expect("absence is not an error")
                .is_none()
        );
    });
}

#[test]
fn corrupt_content_is_an_error_not_a_silent_default() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        let path = seed_raw(b"{ not json");
        let err = portfile::read(&ours).expect_err("a corrupt record must be reported");
        assert_eq!(err.kind(), std::io::ErrorKind::Other);
        let msg = err.to_string();
        assert!(
            msg.contains(&path.display().to_string()),
            "the error names the file to repair: {msg}"
        );
        assert!(
            portfile::preferred_port(&ours).is_err(),
            "the port lookup surfaces the same failure instead of the default port"
        );
    });
}

#[test]
fn a_record_from_another_schema_is_an_error() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        seed_raw(
            br#"{"schema":99,"port":48219,"pid":1,"install_id":"x","config_dir":"/tmp","bound_at_unix":0,"version":"0"}"#,
        );
        let err = portfile::read(&ours).expect_err("an unknown schema is not silently skipped");
        let msg = err.to_string();
        assert!(
            msg.contains("unsupported port record schema 99"),
            "the error names the schema it cannot read: {msg}"
        );
    });
}

#[test]
fn a_record_from_another_install_is_ignored() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        // Why: a config dir copied between machines carries a port that means
        // nothing here, and following it would land on someone else's proxy.
        seed_raw(
            br#"{"schema":1,"port":48219,"pid":1,"install_id":"someone-else","config_dir":"/tmp","bound_at_unix":0,"version":"0"}"#,
        );
        assert!(
            portfile::read(&ours)
                .expect("a foreign record is readable")
                .is_none(),
            "a foreign record is not ours, but it is not corrupt either"
        );
    });
}

#[test]
fn a_record_outside_the_candidate_range_is_an_error() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        // Why: an ephemeral port would become sticky-wrong — preferred next
        // start, different on every restart — so it is refused, not followed.
        let outside = MAX_CANDIDATE_PORT + 1;
        let body = format!(
            r#"{{"schema":1,"port":{outside},"pid":1,"install_id":"{ours}","config_dir":"/tmp","bound_at_unix":0,"version":"0"}}"#
        );
        seed_raw(body.as_bytes());
        let err = portfile::read(&ours).expect_err("an out-of-range port is refused");
        assert!(
            err.to_string().contains("port outside supported range"),
            "{err}"
        );

        for port in [DEFAULT_PROXY_PORT, MAX_CANDIDATE_PORT] {
            portfile::write(port, &ours).expect("record");
            assert_eq!(
                portfile::preferred_port(&ours).expect("readable"),
                Some(port),
                "both ends of the candidate range are accepted"
            );
        }
    });
}

#[test]
fn clear_leaves_a_record_that_is_not_ours() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        portfile::write(48219, &ours).expect("record");
        let path = portfile::portfile_path().expect("path");

        // Rewrite with a different pid: a sibling took over after we crashed.
        let raw = std::fs::read_to_string(&path).expect("read");
        let mine = format!("\"pid\": {}", std::process::id());
        std::fs::write(&path, raw.replace(&mine, "\"pid\": 999999")).expect("rewrite pid");

        portfile::clear(&ours).expect("leaving a sibling's record is not an error");
        assert!(
            path.exists(),
            "clearing must not delete a record another process owns"
        );

        portfile::write(48219, &ours).expect("re-record as us");
        portfile::clear(&ours).expect("our own record clears");
        assert!(!path.exists(), "our own record is removed on shutdown");

        portfile::clear(&ours).expect("clearing an absent record is idempotent");
    });
}

#[test]
fn clear_reports_a_corrupt_record_instead_of_deleting_it() {
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        let path = seed_raw(b"{ not json");
        portfile::clear(&ours)
            .expect_err("a record that cannot be read cannot be verified as ours");
        assert!(
            path.exists(),
            "an unverifiable record is left for the operator, not removed blind"
        );
    });
}

#[cfg(unix)]
#[test]
fn the_record_is_not_world_readable() {
    use std::os::unix::fs::PermissionsExt as _;
    let temp = tempfile::tempdir().expect("config tempdir");
    in_sandbox(&temp, || {
        let ours = establish();
        portfile::write(48219, &ours).expect("record");
        let path = portfile::portfile_path().expect("path");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    });
}
