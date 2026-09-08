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
fn a_placeholder_on_disk_identity_is_re_minted_not_refused() {
    // Why: a placeholder was never durable state; refusing to start over it
    // bricked every command, including the ones that would have repaired it.
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = temp.path().join("systemprompt").join("bridge-install.id");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&path, "unknown\n").expect("seed placeholder id");
    let minted = in_sandbox(&temp, || {
        identity::InstallId::establish().expect("a placeholder id is replaced")
    });
    assert!(minted.is_known(), "the replacement is a real id: {minted}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read").trim(),
        minted.as_str(),
        "the replacement is persisted over the placeholder"
    );

    std::fs::write(&path, "").expect("seed empty id");
    let again = in_sandbox(&temp, || {
        identity::InstallId::establish().expect("an empty id is re-minted")
    });
    assert!(again.is_known());
    assert_ne!(
        again.as_str(),
        minted.as_str(),
        "a fresh nonce, not the old one"
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

#[test]
fn an_ephemeral_identity_is_real_but_never_shared() {
    // Why: a process that cannot read or write the durable id still needs an
    // identity. It must be a real one — an unknown id matches nothing, so a
    // sibling could never be recognised — and it must differ per process, so
    // two identity-less installs never mistake each other for one another.
    let first = identity::InstallId::ephemeral();
    let second = identity::InstallId::ephemeral();
    assert!(first.is_known(), "an ephemeral id is a real nonce: {first}");
    assert!(second.is_known());
    assert_ne!(first.as_str(), second.as_str());
    assert!(
        !first.same_install(&second),
        "two identity-less processes must not read as the same install"
    );
    assert!(
        first.same_install(&first.clone()),
        "an ephemeral id still matches itself within the process"
    );
}
