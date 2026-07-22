use systemprompt_bridge::ids::ProxySecret;
use systemprompt_bridge::proxy::secret;

fn config_sandbox<R>(f: impl FnOnce(&std::path::Path) -> R) -> R {
    let dir = tempfile::tempdir().expect("config sandbox");
    let root = dir.path().display().to_string();
    let path = dir.path().to_path_buf();
    temp_env::with_vars(
        [
            ("XDG_CONFIG_HOME", Some(root.clone())),
            ("HOME", Some(root)),
        ],
        || f(&path),
    )
}

#[test]
fn fingerprint_of_empty_is_marker() {
    assert_eq!(secret::fingerprint(""), "<empty>");
}

#[test]
fn fingerprint_is_eight_lowercase_hex() {
    let fp = secret::fingerprint("a6ee3c83-some-loopback-secret-value");
    assert_eq!(fp.len(), 8);
    assert!(
        fp.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
}

#[test]
fn fingerprint_is_deterministic_and_distinguishes() {
    let a = secret::fingerprint("secret-one");
    let b = secret::fingerprint("secret-one");
    let c = secret::fingerprint("secret-two");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn load_surfaces_an_unreadable_secret_path_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let err = secret::load(dir.path()).expect_err("a directory at the secret path must error");
    assert_ne!(err.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn load_treats_missing_and_blank_files_as_unminted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge-loopback.key");
    assert!(
        secret::load(&path)
            .expect("missing is not an error")
            .is_none()
    );
    std::fs::write(&path, "  \n").unwrap();
    assert!(
        secret::load(&path)
            .expect("blank is not an error")
            .is_none()
    );
}

#[test]
fn for_profile_reports_the_secret_unavailable_until_the_proxy_mints_it() {
    config_sandbox(|_| {
        let err = secret::for_profile().expect_err("no minted secret yet");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(
            err.to_string().contains("proxy has not been started"),
            "{err}"
        );
    });
}

#[cfg(unix)]
#[test]
fn proxy_init_mints_a_private_secret_that_for_profile_then_serves() {
    config_sandbox(|root| {
        let minted = secret::proxy_init().expect("mint succeeds");
        let path = root.join("systemprompt").join("bridge-loopback.key");
        assert!(path.is_file(), "the secret is persisted at {path:?}");

        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the secret file is owner-only");

        let on_disk = secret::load(&path).unwrap().expect("secret readable");
        assert_eq!(on_disk.as_str(), minted.as_str());

        let served = secret::for_profile().expect("for_profile serves the minted secret");
        assert_eq!(served.as_str(), minted.as_str());

        let again = secret::proxy_init().expect("idempotent");
        assert_eq!(again.as_str(), minted.as_str());
    });
}

#[test]
fn verify_is_exact_match_only() {
    let expected = ProxySecret::new("loopback-secret-value");
    assert!(secret::verify("loopback-secret-value", &expected));
    assert!(!secret::verify("loopback-secret-valuX", &expected));
    assert!(!secret::verify("loopback-secret-value-longer", &expected));
    assert!(!secret::verify("", &expected));
}

#[test]
fn reapply_hint_directs_to_reapply_not_client_restart() {
    let hint = secret::reapply_hint();
    assert!(
        hint.contains("re-apply"),
        "hint must direct to re-apply: {hint}"
    );
    assert!(
        !hint.to_ascii_lowercase().contains("restart claude desktop"),
        "hint must not advise restarting the client: {hint}"
    );
}
