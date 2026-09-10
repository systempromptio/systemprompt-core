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
fn load_treats_a_missing_file_as_unminted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge-loopback.key");
    assert!(
        secret::load(&path)
            .expect("missing is not an error")
            .is_none()
    );
}

#[test]
fn load_reports_a_blank_file_instead_of_silently_re_minting() {
    // Why: a blank secret file is a half-written enrollment, not a fresh
    // install; minting over it would hide the failure that produced it.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge-loopback.key");
    std::fs::write(&path, "  \n").unwrap();
    let err = secret::load(&path).expect_err("blank is an error");
    let msg = err.to_string();
    assert!(
        msg.contains("loopback secret is empty") && msg.contains(&path.display().to_string()),
        "the error names the file and the remedy: {msg}"
    );
}

// The before and after of minting, in one test on purpose.
//
// `proxy_init` caches the secret in a process-global `OnceLock`, so a separate
// "not minted yet" test only passes while it happens to be scheduled first.
#[cfg(unix)]
#[test]
fn proxy_init_mints_a_private_secret_that_for_profile_then_serves() {
    config_sandbox(|root| {
        let err = secret::for_profile().expect_err("no minted secret yet");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(
            err.to_string().contains("proxy has not been started"),
            "{err}"
        );

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

#[cfg(unix)]
#[test]
fn reset_replaces_the_secret_and_names_the_stale_profiles() {
    config_sandbox(|root| {
        let first = secret::proxy_init().expect("mint");
        let (second, path) = secret::reset().expect("reset re-mints");
        assert_eq!(path, root.join("systemprompt").join("bridge-loopback.key"));
        assert_ne!(first.as_str(), second.as_str(), "a reset is a new secret");
        let on_disk = secret::load(&path).unwrap().expect("readable");
        assert_eq!(on_disk.as_str(), second.as_str());
    });
}

// Why: the incident this guards against was a key file the user could no
// longer read; the proxy failed with a bare "Access is denied". The file is
// this user's to delete, so a fresh key is minted and the profiles pick it up
// on the next sync.
#[cfg(unix)]
#[test]
fn an_unreadable_secret_that_can_be_removed_is_minted_afresh() {
    use std::os::unix::fs::PermissionsExt;
    config_sandbox(|root| {
        let dir = root.join("systemprompt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bridge-loopback.key");
        std::fs::write(&path, "sealed").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let minted = secret::proxy_init().expect("unreadable but removable re-mints");
        let on_disk = secret::load(&path).unwrap().expect("fresh key is readable");
        assert_eq!(on_disk.as_str(), minted.as_str());
        assert_ne!(
            on_disk.as_str(),
            "sealed",
            "the unreadable file was replaced"
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600,
            "the fresh key is private"
        );
    });
}

// Why: only a file that cannot be removed either is left to the operator, and
// then the error must name the file and the remedy rather than "Access is
// denied".
#[cfg(unix)]
#[test]
fn an_unreadable_secret_that_cannot_be_removed_fails_with_the_file_and_the_remedy() {
    use std::os::unix::fs::PermissionsExt;
    config_sandbox(|root| {
        let dir = root.join("systemprompt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bridge-loopback.key");
        std::fs::write(&path, "sealed").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
        let err = secret::proxy_init().expect_err("undeletable must not mint over the file");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        let msg = err.to_string();
        assert!(
            msg.contains(&path.display().to_string())
                && msg.contains("could not be removed either")
                && msg.contains("Reset local proxy secret"),
            "{msg}"
        );
        assert_eq!(
            secret::load(&path).unwrap().unwrap().as_str(),
            "sealed",
            "the file was left alone"
        );
    });
}
