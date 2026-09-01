use systemprompt_bridge::auth::{cache, setup};
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_identifiers::ValidatedUrl;
use tempfile::TempDir;

const GOOD: &str = "sp-live-testprefix.secretsecretsecretsecretsecret012345";

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

#[test]
fn a_token_minted_for_another_gateway_is_refused_and_discarded() {
    let ((first, second), _dirs) = sandbox(|| {
        let issuer = url("http://gw-a.invalid:8080");
        cache::write(&issuer, &token(3600)).expect("write");
        let other = cache::read_valid(&url("http://gw-b.invalid:8080"));
        let same = cache::read_valid(&issuer);
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
        cache::write(&issuer, &token(3600)).expect("write");
        cache::read_valid(&issuer)
    });
    assert!(found.is_some(), "the issuing gateway still reads its token");
}

#[test]
fn login_discards_a_cached_token_so_the_new_credential_takes_effect() {
    let (cached, _dirs) = sandbox(|| {
        let gateway = url("http://gw.invalid:8080");
        cache::write(&gateway, &token(3600)).expect("write");
        setup::login(GOOD, Some(gateway.as_str())).expect("login");
        cache::read_valid(&gateway)
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
        cache::write(&gateway, &token(3600)).expect("write");
        setup::set_gateway_url("http://gw-b.invalid:8080").expect("set gateway");
        cache::read_valid(&gateway)
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
