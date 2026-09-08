use systemprompt_bridge::auth::cache::CredentialBinding;
use systemprompt_bridge::auth::{cache, setup};
use systemprompt_bridge::config;
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_identifiers::ValidatedUrl;
use tempfile::TempDir;

const GOOD: &str = "sp-live-testprefix.secretsecretsecretsecretsecret012345";
const OTHER: &str = "sp-live-otherprefix.secretsecretsecretsecretsecret012345";

fn sandbox<R>(f: impl FnOnce() -> R) -> (R, [TempDir; 3]) {
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
    (out, [config, state, home])
}

fn url(s: &str) -> ValidatedUrl {
    ValidatedUrl::try_new(s).expect("valid url")
}

fn token(ttl: u64) -> HelperOutput {
    HelperOutput {
        token: systemprompt_bridge::ids::BearerToken::new("header.payload.signature"),
        ttl,
        headers: std::collections::HashMap::new(),
    }
}

fn write_token(gateway: &ValidatedUrl, ttl: u64) {
    let cfg = config::load().expect("config");
    let binding = CredentialBinding::capture(&cfg).expect("binding");
    cache::write_bound(gateway, &token(ttl), &binding).expect("write_bound");
}

fn cache_file() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var_os("XDG_CACHE_HOME").expect("XDG_CACHE_HOME"))
        .join(systemprompt_bridge::brand::brand().working_dir_name)
        .join("cache.json")
}

#[test]
fn a_token_minted_for_another_gateway_is_refused_and_discarded() {
    let ((first, second), _dirs) = sandbox(|| {
        let issuer = url("http://gw-a.invalid:8080");
        setup::login(GOOD, Some(issuer.as_str())).expect("login");
        write_token(&issuer, 3600);
        let other = cache::read_valid(&url("http://gw-b.invalid:8080")).expect("read");
        let same = cache::read_valid(&issuer).expect("read");
        (other, same)
    });
    assert!(
        first.is_none(),
        "a token minted for gateway A must not be replayed at gateway B"
    );
    assert!(
        second.is_none(),
        "the mismatched entry is deleted, not merely skipped, so it cannot be reused"
    );
}

#[test]
fn a_token_is_returned_for_the_gateway_that_minted_it() {
    let (found, _dirs) = sandbox(|| {
        let issuer = url("http://gw-a.invalid:8080");
        setup::login(GOOD, Some(issuer.as_str())).expect("login");
        write_token(&issuer, 3600);
        cache::read_valid(&issuer).expect("read")
    });
    let found = found.expect("the issuing gateway still reads its token");
    assert_eq!(found.token.as_str(), "header.payload.signature");
}

#[test]
fn a_token_bound_to_a_replaced_credential_is_refused_and_discarded() {
    let ((swapped, after), _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        let paths = setup::login(GOOD, Some(gateway.as_str())).expect("login");
        write_token(&gateway, 3600);
        std::fs::write(&paths.pat_file, OTHER).expect("swap PAT");
        let swapped = cache::read_valid(&gateway).expect("read");
        let after = cache_file().exists();
        (swapped, after)
    });
    assert!(
        swapped.is_none(),
        "a token minted from one PAT must not be replayed once the PAT changes underneath it"
    );
    assert!(
        !after,
        "the entry bound to the old credential is deleted, not merely skipped"
    );
}

#[test]
fn a_binding_captured_before_the_credential_changed_cannot_be_written() {
    let (err, _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        let paths = setup::login(GOOD, Some(gateway.as_str())).expect("login");
        let binding =
            CredentialBinding::capture(&config::load().expect("config")).expect("binding");
        std::fs::write(&paths.pat_file, OTHER).expect("swap PAT");
        let err = cache::write_bound(&gateway, &token(3600), &binding).expect_err("refused");
        assert!(
            !cache_file().exists(),
            "a refused write leaves no entry behind"
        );
        err
    });
    assert!(
        err.to_string().contains("credentials changed"),
        "the write names the race it refuses: {err}"
    );
}

#[test]
fn a_binding_captured_for_another_gateway_cannot_be_written() {
    let (err, _dirs) = sandbox(|| {
        let issuer = url("http://gw-a.invalid:8080");
        setup::login(GOOD, Some(issuer.as_str())).expect("login");
        let binding =
            CredentialBinding::capture(&config::load().expect("config")).expect("binding");
        cache::write_bound(&url("http://gw-b.invalid:8080"), &token(3600), &binding)
            .expect_err("refused")
    });
    assert!(
        err.to_string().contains("gateway or credentials changed"),
        "a binding for gateway A must not vouch for a token stored under gateway B: {err}"
    );
}

#[test]
fn a_binding_cannot_be_captured_without_a_credential() {
    let (err, _dirs) = sandbox(|| {
        CredentialBinding::capture(&config::load().expect("config")).expect_err("no credential")
    });
    assert!(
        err.to_string()
            .contains("no credential identity configured"),
        "an unbound token would survive any later sign-in: {err}"
    );
}

#[test]
fn a_legacy_session_without_a_generation_cannot_bind_a_token() {
    let (err, _dirs) = sandbox(|| {
        let cfg = config::Config {
            gateway_url: Some(url("http://gw.invalid:8080")),
            session: Some(config::SessionConfig {
                generation: None,
                enabled: Some(true),
            }),
            ..config::Config::default()
        };
        CredentialBinding::capture(&cfg).expect_err("legacy session")
    });
    assert!(
        err.to_string().contains("sign in again"),
        "a pre-generation session has no identity to bind to: {err}"
    );
}

#[test]
fn a_malformed_cache_is_an_error_not_a_miss() {
    let ((read, gateway), _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        let path = cache_file();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("cache dir");
        std::fs::write(&path, b"{not json").expect("corrupt cache");
        (cache::read_valid(&gateway), cache::cached_gateway())
    });
    let err = read.expect_err("a corrupt cache must not be mistaken for an empty one");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("sign in again"),
        "the error tells the operator how to recover: {err}"
    );
    assert_eq!(
        gateway
            .expect_err("cached_gateway reads the same file")
            .kind(),
        std::io::ErrorKind::InvalidData
    );
}

#[test]
fn a_missing_cache_is_a_miss_not_an_error() {
    let ((read, gateway), _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        (cache::read_valid(&gateway), cache::cached_gateway())
    });
    assert!(read.expect("read").is_none());
    assert!(gateway.expect("cached_gateway").is_none());
}

#[test]
fn login_discards_a_cached_token_so_the_new_credential_takes_effect() {
    let (cached, _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        write_token(&gateway, 3600);
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        cache::read_valid(&gateway).expect("read")
    });
    assert!(
        cached.is_none(),
        "a stale JWT outliving login is what makes a re-login a no-op"
    );
}

#[test]
fn set_gateway_url_discards_a_cached_token() {
    let (cached, _dirs) = sandbox(|| {
        let gateway = url("http://gw-a.invalid:8080");
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        write_token(&gateway, 3600);
        setup::set_gateway_url("http://gw-b.invalid:8080").expect("set gateway");
        cache::read_valid(&gateway).expect("read")
    });
    assert!(
        cached.is_none(),
        "repointing the bridge drops the old token"
    );
}

#[test]
fn re_login_preserves_unrelated_config_sections() {
    let (config, _dirs) = sandbox(|| {
        let paths = setup::login(GOOD, Some("http://gw.invalid:8080")).expect("first login");
        let existing = std::fs::read_to_string(&paths.config_file).expect("config");
        std::fs::write(
            &paths.config_file,
            format!("{existing}\n[sync]\npinned_pubkey = \"abc123\"\n\n[claude]\norganization_uuid = \"org-1\"\n"),
        )
        .expect("augment config");
        setup::login(GOOD, None).expect("second login");
        std::fs::read_to_string(&paths.config_file).expect("config")
    });
    let parsed: toml::Value = toml::from_str(&config).expect("valid TOML");
    assert_eq!(
        parsed["sync"]["pinned_pubkey"].as_str(),
        Some("abc123"),
        "a re-login that drops the pinned pubkey silently re-enables trust-on-first-use: {config}"
    );
    assert_eq!(
        parsed["claude"]["organization_uuid"].as_str(),
        Some("org-1"),
        "unrelated sections survive a re-login: {config}"
    );
}

#[test]
fn login_after_a_session_sign_in_removes_the_session_section() {
    let (config, _dirs) = sandbox(|| {
        setup::session_setup(Some("http://gw.invalid:8080")).expect("session setup");
        let paths = setup::login(GOOD, None).expect("login");
        std::fs::read_to_string(&paths.config_file).expect("config")
    });
    let parsed: toml::Value = toml::from_str(&config).expect("valid TOML");
    assert!(
        parsed.get("session").is_none(),
        "the superseded credential must not stay in the auth chain: {config}"
    );
    assert!(
        parsed.get("pat").is_some(),
        "the new PAT is written: {config}"
    );
}
