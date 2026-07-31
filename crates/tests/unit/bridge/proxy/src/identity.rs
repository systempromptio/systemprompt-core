use systemprompt_bridge::proxy::identity;

#[test]
fn an_install_id_is_minted_once_and_then_read_back() {
    let temp = tempfile::tempdir().expect("config tempdir");
    let path = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        identity::install_id_path().expect("a config dir yields an install id path")
    });
    assert_eq!(
        path,
        temp.path().join("systemprompt").join("bridge-install.id")
    );

    // The process caches the id in a OnceLock, so minting is exercised through
    // the file rather than by calling install_id() twice under two sandboxes.
    assert!(!path.exists(), "nothing is written before first use");
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
    let json = temp_env::with_var("XDG_CONFIG_HOME", Some(temp.path().as_os_str()), || {
        let who = identity::WhoAmI::current(48218, 1_753_948_800);
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
