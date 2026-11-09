use std::path::{Path, PathBuf};

use systemprompt_bridge::auth::cache::{
    CredentialBinding, cached_gateway, is_no_credential_identity, read_for, write_bound,
};
use systemprompt_bridge::config;
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;

fn cache_file(cache_home: &Path) -> PathBuf {
    cache_home.join("systemprompt-bridge").join("cache.json")
}

fn in_sandbox(pat: Option<&str>, f: impl FnOnce(&Path)) {
    let config_home = tempfile::tempdir().expect("config tempdir");
    let cache_home = tempfile::tempdir().expect("cache tempdir");
    temp_env::with_vars(
        [
            (
                "XDG_CONFIG_HOME",
                Some(config_home.path().as_os_str().to_owned()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(cache_home.path().as_os_str().to_owned()),
            ),
            ("SP_BRIDGE_PAT", pat.map(Into::into)),
        ],
        || f(cache_home.path()),
    );
}

#[test]
fn a_missing_cache_file_is_a_miss() {
    in_sandbox(Some("sp-live-a.b"), |_| {
        assert!(
            cached_gateway()
                .expect("a missing cache is not an error")
                .is_none(),
            "no file means no cached gateway"
        );
    });
}

#[test]
fn a_malformed_cache_file_is_discarded_as_a_miss() {
    in_sandbox(Some("sp-live-a.b"), |cache_home| {
        let path = cache_file(cache_home);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("cache dir");
        std::fs::write(&path, b"{not json").expect("seed malformed cache");

        let cfg = config::load().expect("config");
        let gateway = config::gateway_url_or_default(&cfg);
        let read = read_for(&cfg, &gateway, 0).expect("a malformed cache is not an error");
        assert!(read.is_none(), "a malformed cache serves nothing");
        assert!(!path.exists(), "the malformed cache is removed");
    });
}

#[test]
fn a_bound_token_round_trips_through_the_cache() {
    in_sandbox(Some("sp-live-a.b"), |_| {
        let cfg = config::load().expect("config");
        let gateway = config::gateway_url_or_default(&cfg);
        let binding = CredentialBinding::capture(&cfg).expect("the PAT is an identity");
        let output = HelperOutput {
            token: BearerToken::new("cached-jwt"),
            ttl: 3600,
            headers: Default::default(),
            device_token: None,
        };
        write_bound(&cfg, &gateway, &output, &binding).expect("write cache");

        let read = read_for(&cfg, &gateway, 60)
            .expect("read cache")
            .expect("the token just written is served");
        assert_eq!(read.token.as_str(), "cached-jwt");
        assert_eq!(cached_gateway().expect("read gateway"), Some(gateway));
    });
}

#[test]
fn no_configured_credential_is_reported_as_a_missing_identity() {
    in_sandbox(None, |_| {
        let cfg = config::load().expect("config");
        let err = CredentialBinding::capture(&cfg).expect_err("nothing to bind to");
        assert!(
            is_no_credential_identity(&err),
            "a signed-out bridge is distinguishable by type: {err}"
        );
    });
}

#[test]
fn an_io_failure_is_not_a_missing_identity() {
    let err = std::io::Error::other("read PAT: permission denied");
    assert!(!is_no_credential_identity(&err));
}
