use systemprompt_bridge::auth::setup::{self, SetupError};
use tempfile::TempDir;

const GOOD: &str = "sp-live-testprefix.secretsecretsecretsecretsecret012345";

fn sandbox<R>(f: impl FnOnce() -> R) -> (R, TempDir) {
    let config = TempDir::new().expect("config tempdir");
    let state = TempDir::new().expect("state tempdir");
    let home = TempDir::new().expect("home tempdir");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(home.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config.path().display().to_string())),
        ("XDG_STATE_HOME", Some(state.path().display().to_string())),
        ("XDG_CACHE_HOME", Some(home.path().display().to_string())),
    ];
    let out = temp_env::with_vars(vars, f);
    drop((state, home));
    (out, config)
}

#[test]
fn login_writes_a_private_pat_file_and_a_config_pointing_at_it() {
    let (paths, _cfg) = sandbox(|| setup::login(GOOD, None).expect("login succeeds"));

    let stored = std::fs::read_to_string(&paths.pat_file).expect("pat file");
    assert_eq!(stored, GOOD);
    let config = std::fs::read_to_string(&paths.config_file).expect("config file");
    assert!(
        config.contains(&paths.pat_file.display().to_string()),
        "config points at the PAT file: {config}"
    );
    assert!(
        config.contains("http://localhost:8080"),
        "the brand default gateway is written when none is supplied: {config}"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&paths.pat_file)
            .expect("stat pat")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "PAT file is owner-only");
        let dir_mode = std::fs::metadata(&paths.config_dir)
            .expect("stat dir")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700, "config dir is owner-only");
    }
}

#[test]
fn a_second_login_without_a_gateway_keeps_the_first_one() {
    let (config, _cfg) = sandbox(|| {
        setup::login(GOOD, Some("http://gw.invalid:7000")).expect("first login");
        let paths = setup::login(GOOD, None).expect("second login");
        std::fs::read_to_string(&paths.config_file).expect("config")
    });
    assert!(
        config.contains("http://gw.invalid:7000"),
        "an existing gateway_url survives a re-login: {config}"
    );
}

#[test]
fn token_validation_rejects_the_three_malformed_shapes() {
    let ((prefix, dot, short), _cfg) = sandbox(|| {
        (
            setup::login("live-nope.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", None),
            setup::login("sp-live-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", None),
            setup::login("sp-live-a.b", None),
        )
    });
    assert!(
        matches!(prefix, Err(SetupError::Token(ref m)) if m.contains("sp-live-")),
        "{prefix:?}"
    );
    assert!(
        matches!(dot, Err(SetupError::Token(ref m)) if m.contains('.')),
        "{dot:?}"
    );
    assert!(
        matches!(short, Err(SetupError::Token(ref m)) if m.contains("too short")),
        "{short:?}"
    );
}

#[test]
fn set_gateway_url_rewrites_the_config_and_rejects_an_empty_value() {
    let ((config, empty), _cfg) = sandbox(|| {
        setup::login(GOOD, None).expect("login");
        let paths = setup::set_gateway_url("  http://moved.invalid:7100  ").expect("set gateway");
        (
            std::fs::read_to_string(&paths.config_file).expect("config"),
            setup::set_gateway_url("   "),
        )
    });
    assert!(
        config.contains("gateway_url = \"http://moved.invalid:7100\""),
        "the trimmed override is written: {config}"
    );
    assert!(matches!(empty, Err(SetupError::Path(_))), "{empty:?}");
}

#[test]
fn session_setup_writes_a_session_section_and_no_pat_section() {
    let ((config, pat_exists), _cfg) = sandbox(|| {
        let paths = setup::session_setup(Some("http://gw.invalid:7200")).expect("session setup");
        (
            std::fs::read_to_string(&paths.config_file).expect("config"),
            paths.pat_file.exists(),
        )
    });
    assert!(config.contains("[session]"), "{config}");
    assert!(config.contains("enabled = true"), "{config}");
    assert!(
        !config.contains("[pat]"),
        "the session flow stores no long-lived secret: {config}"
    );
    assert!(!pat_exists, "no PAT file is created by the session flow");
}

#[test]
fn session_setup_inherits_the_gateway_already_on_disk() {
    let (config, _cfg) = sandbox(|| {
        setup::login(GOOD, Some("http://gw.invalid:7300")).expect("login");
        let paths = setup::session_setup(None).expect("session setup");
        std::fs::read_to_string(&paths.config_file).expect("config")
    });
    assert!(
        config.contains("http://gw.invalid:7300"),
        "the persisted gateway is reused: {config}"
    );
}

#[test]
fn logout_keeps_a_config_that_still_carries_a_gateway() {
    let ((config, pat_exists), _cfg) = sandbox(|| {
        setup::login(GOOD, Some("http://gw.invalid:7400")).expect("login");
        let paths = setup::logout().expect("logout");
        (
            std::fs::read_to_string(&paths.config_file).expect("config survives"),
            paths.pat_file.exists(),
        )
    });
    assert!(!pat_exists, "logout removes the PAT file");
    assert!(
        !config.contains("[pat]"),
        "[pat] section stripped: {config}"
    );
    assert!(
        config.contains("http://gw.invalid:7400"),
        "gateway_url survives logout: {config}"
    );
}

#[test]
fn clean_reports_exactly_what_it_removed() {
    let ((first, second), _cfg) = sandbox(|| {
        setup::login(GOOD, None).expect("login");
        let first = setup::clean().expect("first clean");
        let second = setup::clean().expect("second clean");
        (first, second)
    });
    assert!(first.pat_removed && first.config_removed);
    assert!(!first.paths.pat_file.exists());
    assert!(!first.paths.config_file.exists());
    assert!(
        !second.pat_removed && !second.config_removed,
        "a second clean has nothing left to remove"
    );
}

#[test]
fn status_tracks_the_login_lifecycle() {
    let ((before, after, cleaned), _cfg) = sandbox(|| {
        let before = setup::status().expect("status");
        setup::login(GOOD, None).expect("login");
        let after = setup::status().expect("status");
        setup::clean().expect("clean");
        let cleaned = setup::status().expect("status");
        (before, after, cleaned)
    });
    assert!(!before.config_present && !before.pat_present);
    assert!(after.config_present && after.pat_present);
    assert!(!cleaned.config_present && !cleaned.pat_present);
    assert!(
        !before.oauth_creds_present,
        "no OAuth client creds in a fresh sandbox"
    );
}

#[test]
fn logout_on_a_config_that_is_only_a_pat_section_removes_the_file() {
    let ((exists, _), _cfg) = sandbox(|| {
        let paths = setup::resolve_paths().expect("paths");
        std::fs::create_dir_all(&paths.config_dir).expect("mkdir");
        std::fs::write(&paths.config_file, "[pat]\nfile = \"/nowhere\"\n").expect("seed config");
        let out = setup::logout().expect("logout");
        (out.config_file.exists(), out)
    });
    assert!(
        !exists,
        "a config left empty after stripping [pat] is deleted"
    );
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn login_fails_with_the_create_dir_context_when_the_config_base_is_read_only() {
    let (err, cfg) = sandbox(|| {
        let base = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
        set_mode(&base, 0o555);
        setup::login(GOOD, None).expect_err("login under a read-only config base must fail")
    });
    set_mode(cfg.path(), 0o755);
    match err {
        SetupError::Io(msg) => assert!(
            msg.contains("create config dir"),
            "error names the failing step: {msg}"
        ),
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn login_fails_with_the_rename_context_when_the_pat_path_is_a_directory() {
    let (err, _cfg) = sandbox(|| {
        let paths = setup::resolve_paths().unwrap();
        std::fs::create_dir_all(&paths.pat_file).unwrap();
        setup::login(GOOD, None).expect_err("a directory squatting on the PAT path must fail")
    });
    match err {
        SetupError::Io(msg) => assert!(
            msg.contains("rename"),
            "the atomic-write rename step surfaces: {msg}"
        ),
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn logout_fails_with_the_read_config_context_when_the_config_path_is_a_directory() {
    let (err, _cfg) = sandbox(|| {
        let paths = setup::resolve_paths().unwrap();
        std::fs::create_dir_all(&paths.config_file).unwrap();
        setup::logout().expect_err("an unreadable config must fail logout")
    });
    match err {
        SetupError::Io(msg) => assert!(
            msg.contains("read config"),
            "error names the failing step: {msg}"
        ),
        other => panic!("expected Io, got {other:?}"),
    }
}

#[test]
fn clean_fails_with_the_remove_context_when_the_pat_path_is_a_directory() {
    let (err, _cfg) = sandbox(|| {
        let paths = setup::resolve_paths().unwrap();
        std::fs::create_dir_all(&paths.pat_file).unwrap();
        setup::clean().expect_err("an unremovable PAT path must fail clean")
    });
    match err {
        SetupError::Io(msg) => assert!(
            msg.contains("remove"),
            "error names the failing step: {msg}"
        ),
        other => panic!("expected Io, got {other:?}"),
    }
}
