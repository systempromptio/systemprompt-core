use systemprompt_bridge::proxy::identity;

fn in_sandbox<T>(temp: &tempfile::TempDir, f: impl FnOnce() -> T) -> T {
    temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), f)
}

#[test]
fn an_install_id_is_minted_once_and_then_read_back() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = in_sandbox(&temp, || {
        identity::install_id_path().expect("a config dir yields an install id path")
    });
    assert_eq!(
        path,
        temp.path().join("systemprompt").join("bridge-install.id")
    );
    assert!(!path.exists(), "nothing is written before first use");

    let (first, second) = in_sandbox(&temp, || {
        let first = identity::InstallId::establish().expect("first use mints an id");
        let second = identity::InstallId::establish().expect("second use reads it back");
        (first, second)
    });
    assert!(first.is_known());
    assert!(
        first.same_install(&second),
        "the minted id is persisted and re-read, not re-minted: {first} vs {second}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("id file").trim(),
        first.as_str()
    );
}

#[test]
fn an_invalid_on_disk_identity_is_an_error_not_an_unknown_install() {
    // Why: an install that silently ran as "unknown" would never match its own
    // port record or whoami, so every start would look like a foreign proxy.
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = temp.path().join("systemprompt").join("bridge-install.id");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "unknown\n").expect("seed placeholder id");
    let err = in_sandbox(&temp, || {
        identity::InstallId::establish().expect_err("a placeholder id is refused")
    });
    let msg = err.to_string();
    assert!(
        msg.contains("invalid install identity") && msg.contains(&path.display().to_string()),
        "the error names the file to repair: {msg}"
    );

    std::fs::write(&path, "").expect("seed empty id");
    let err = in_sandbox(&temp, || {
        identity::InstallId::establish().expect_err("an empty id is refused, not re-minted")
    });
    assert!(
        err.to_string().contains("invalid install identity"),
        "{err}"
    );
}

#[test]
fn an_unknown_id_is_never_treated_as_a_match() {
    // Why: two installs that both failed to establish an id must not read as
    // each other, or one would stand aside for a stranger holding its port.
    assert!(!identity::is_known("unknown"));
    assert!(!identity::is_known(""));
    assert!(identity::is_known("9f2c41ab77e0d315"));
}

#[test]
fn the_whoami_payload_carries_no_secret_material() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let json = in_sandbox(&temp, || {
        let ours = identity::InstallId::establish().expect("the sandbox mints an id");
        let who = identity::WhoAmI::current(48218, 1_753_948_800, &ours);
        serde_json::to_string(&who).expect("whoami serialises")
    });

    assert!(json.contains("\"port\":48218"));
    assert!(json.contains("systemprompt-bridge"));
    // A caller able to confirm a guessed secret would turn this unauthenticated
    // endpoint into an oracle, so these must never appear.
    for forbidden in [
        "secret",
        "fingerprint",
        "bridge-loopback.key",
        "gateway",
        "token",
    ] {
        assert!(
            !json.contains(forbidden),
            "whoami leaked `{forbidden}`: {json}"
        );
    }
}
