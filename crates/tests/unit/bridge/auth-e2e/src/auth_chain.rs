use std::io::Write;

use systemprompt_bridge::auth::{self, ChainError, cache};
use systemprompt_bridge::config::{Config, PatConfig, SessionConfig};
use systemprompt_identifiers::{SessionId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sandbox_vars(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
        ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
    ]
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn auth_response_body() -> serde_json::Value {
    serde_json::json!({
        "token": "sp-bearer-deadbeef-token-value",
        "ttl": 3600,
        "headers": { "x-session-id": "ignored" }
    })
}

#[test]
fn has_credential_source_true_for_pat_file() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        let pat_path = home.path().join("pat.txt");
        std::fs::write(&pat_path, "sp-live-abc").unwrap();
        let cfg = Config {
            pat: Some(PatConfig {
                file: Some(pat_path.to_string_lossy().into_owned()),
            }),
            ..Config::default()
        };
        assert!(auth::has_credential_source(&cfg));
    });
}

#[test]
fn has_credential_source_false_for_empty_config() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        let cfg = Config::default();
        assert!(!auth::has_credential_source(&cfg));
    });
}

#[test]
fn has_credential_source_true_for_enabled_session() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        let cfg = Config {
            session: Some(SessionConfig {
                generation: None,
                enabled: Some(true),
            }),
            ..Config::default()
        };
        assert!(auth::has_credential_source(&cfg));
    });
}

fn config_file(home: &TempDir) -> std::path::PathBuf {
    home.path().join("bridge.toml")
}

fn sandbox_vars_with_config(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let mut vars = sandbox_vars(home);
    vars.retain(|(name, _)| *name != "SP_BRIDGE_CONFIG");
    vars.push((
        "SP_BRIDGE_CONFIG",
        Some(config_file(home).to_string_lossy().into_owned()),
    ));
    vars
}

fn persist_config(home: &TempDir, gateway: &str, pat_path: &std::path::Path) {
    let path = config_file(home);
    // A literal string: a path's backslashes would be invalid escapes in a
    // basic string.
    std::fs::write(
        &path,
        format!(
            "gateway_url = \"{gateway}\"\n[pat]\nfile = '{}'\n",
            pat_path.display()
        ),
    )
    .unwrap();
}

#[test]
fn pat_provider_happy_path_yields_bearer_and_binds_the_cache_entry() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars_with_config(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(ResponseTemplate::new(200).set_body_json(auth_response_body()))
                .mount(&server)
                .await;

            let pat_path = home.path().join("pat.txt");
            let mut f = std::fs::File::create(&pat_path).unwrap();
            writeln!(f, "sp-live-secret-pat").unwrap();

            let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
            let cfg = Config {
                gateway_url: Some(gateway.clone()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };
            persist_config(&home, &server.uri(), &pat_path);

            let out = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect("PAT exchange should succeed");
            assert_eq!(out.token.expose(), "sp-bearer-deadbeef-token-value");
            assert_eq!(out.ttl, 3600);

            let cached = cache::read_for(&cfg, &gateway, 30)
                .expect("cache readable")
                .expect("the minted token is cached for the PAT that minted it");
            assert_eq!(cached.token.expose(), "sp-bearer-deadbeef-token-value");

            std::fs::write(&pat_path, "sp-live-a-different-pat").unwrap();
            assert!(
                cache::read_for(&cfg, &gateway, 30)
                    .expect("cache readable")
                    .is_none(),
                "the entry is bound to the PAT that minted it, not just the gateway"
            );
        });
    });
}

#[test]
fn acquire_bearer_caches_against_the_config_it_was_given_not_the_one_on_disk() {
    // Why: the GUI and the proxy mint against an in-memory config; binding the
    // cache to a fresh `config::load()` instead reported "credentials changed"
    // for a credential that never changed, so nothing minted that way was
    // ever cached.
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(ResponseTemplate::new(200).set_body_json(auth_response_body()))
                .mount(&server)
                .await;

            let pat_path = home.path().join("pat.txt");
            std::fs::write(&pat_path, "sp-live-secret-pat").unwrap();
            let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
            let cfg = Config {
                gateway_url: Some(gateway.clone()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let out = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect("an in-memory config mints and caches like a loaded one");
            let cached = cache::read_for(&cfg, &gateway, 30)
                .expect("cache readable")
                .expect("the token is cached under the config that minted it");
            assert_eq!(cached.token.expose(), out.token.expose());

            let on_disk = Config::default();
            let err = cache::read_for(&on_disk, &gateway, 30)
                .expect_err("a config with no credential cannot claim the entry");
            assert!(
                err.to_string()
                    .contains("no credential identity configured"),
                "{err}"
            );
        });
    });
}

#[test]
fn pat_exchange_http_failure_names_the_failed_provider() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&server)
                .await;

            let pat_path = home.path().join("pat.txt");
            std::fs::write(&pat_path, "sp-live-secret-pat").unwrap();

            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new(server.uri()).unwrap()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let err = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect_err("401 must not yield a bearer");
            assert!(
                err.is_terminal(),
                "a 401 is the gateway rejecting the credential; retrying cannot help: {err:?}"
            );
            match err {
                ChainError::Providers { failures, .. } => assert_eq!(
                    failures,
                    vec!["pat: gateway returned status 401 Unauthorized from pat".to_owned()],
                    "the rejected provider is named so the operator knows which credential failed"
                ),
                other => panic!("expected ChainError::Providers, got {other:?}"),
            }
        });
    });
}

#[test]
fn pat_exchange_server_error_names_the_failed_provider() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&server)
                .await;

            let pat_path = home.path().join("pat.txt");
            std::fs::write(&pat_path, "sp-live-secret-pat").unwrap();

            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new(server.uri()).unwrap()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let err = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect_err("500 must not yield a bearer");
            assert!(
                !err.is_terminal(),
                "a 500 is the gateway being unwell, not a bad credential: {err:?}"
            );
            match err {
                ChainError::Providers { failures, .. } => {
                    assert_eq!(failures.len(), 1, "{failures:?}");
                    assert!(
                        failures[0].starts_with("pat: ") && failures[0].contains("500"),
                        "{failures:?}"
                    );
                },
                other => panic!("expected ChainError::Providers, got {other:?}"),
            }
        });
    });
}

#[test]
fn pat_exchange_transport_failure_is_not_terminal() {
    // Why: the 2026-09-10 astound incident. A refresh tick fired one second before
    // the laptop finished waking, the PAT exchange failed to reach the gateway, and
    // a terminal verdict here latched the bridge into "sign in required" for five
    // hours with a perfectly valid PAT still on disk.
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let pat_path = home.path().join("pat.txt");
            std::fs::write(&pat_path, "sp-live-secret-pat").unwrap();

            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new("http://127.0.0.1:1").unwrap()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let err = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect_err("an unreachable gateway must not yield a bearer");
            assert!(
                !err.is_terminal(),
                "an unreachable gateway must stay retryable so the next tick recovers: {err:?}"
            );
        });
    });
}

#[test]
fn no_credential_source_fails_before_the_chain_runs() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new("http://127.0.0.1:1").unwrap()),
                ..Config::default()
            };
            let err = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect_err("no provider configured must fail");
            assert!(
                matches!(err, ChainError::NoneSucceeded),
                "no provider is consulted and no binding is captured without a credential: {err:?}"
            );
        });
    });
}
// Append to crates/tests/unit/bridge/auth-e2e/src/auth_chain.rs.
fn pat_config(home: &TempDir, gateway: &str) -> Config {
    let pat_path = home.path().join("pat.txt");
    std::fs::write(&pat_path, "sp-live-session-bound-pat").unwrap();
    Config {
        gateway_url: Some(ValidatedUrl::try_new(gateway).unwrap()),
        pat: Some(PatConfig {
            file: Some(pat_path.to_string_lossy().into_owned()),
        }),
        ..Config::default()
    }
}

fn cache_file(home: &TempDir) -> std::path::PathBuf {
    home.path()
        .join(".cache")
        .join(systemprompt_bridge::brand::brand().working_dir_name)
        .join("cache.json")
}

fn session_bound_auth_response(request: &wiremock::Request) -> ResponseTemplate {
    let session = request
        .headers
        .get("x-session-id")
        .and_then(|value| value.to_str().ok())
        .expect("PAT exchange carries x-session-id");
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "token": format!("sp-bearer-deadbeef-{session}-token-value"),
        "ttl": 3600,
        "headers": { "x-session-id": session }
    }))
}

#[test]
fn read_or_refresh_reuses_only_the_session_that_minted_the_cached_token() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(session_bound_auth_response)
                .expect(2)
                .mount(&server)
                .await;
            let cfg = pat_config(&home, &server.uri());
            let first_session = SessionId::new("session-a");
            let second_session = SessionId::new("session-b");
            let first = auth::read_or_refresh(&cfg, 30, &first_session, &reqwest::Client::new())
                .await
                .expect("first session mints");
            let cached = auth::read_or_refresh(&cfg, 30, &first_session, &reqwest::Client::new())
                .await
                .expect("same session reuses cache");
            assert_eq!(cached.token.expose(), first.token.expose());
            let second = auth::read_or_refresh(&cfg, 30, &second_session, &reqwest::Client::new())
                .await
                .expect("different session remints");
            assert_ne!(second.token.expose(), first.token.expose());
            let requests = server.received_requests().await.expect("recorded mints");
            assert_eq!(requests.len(), 2);
            assert_eq!(
                requests[0]
                    .headers
                    .get("x-session-id")
                    .and_then(|value| value.to_str().ok()),
                Some("session-a")
            );
            assert_eq!(
                requests[1]
                    .headers
                    .get("x-session-id")
                    .and_then(|value| value.to_str().ok()),
                Some("session-b")
            );
            for request in requests {
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&request.body).unwrap(),
                    serde_json::json!({})
                );
            }
        });
    });
}

#[test]
fn read_or_refresh_replaces_a_corrupt_cache_with_a_fresh_session_bound_entry() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(session_bound_auth_response)
                .expect(2)
                .mount(&server)
                .await;
            let cfg = pat_config(&home, &server.uri());
            let original = SessionId::new("corrupt-original");
            auth::read_or_refresh(&cfg, 30, &original, &reqwest::Client::new())
                .await
                .expect("seed cache");
            let path = cache_file(&home);
            std::fs::write(&path, b"{broken").unwrap();
            let repaired_session = SessionId::new("corrupt-repair");
            auth::read_or_refresh(&cfg, 30, &repaired_session, &reqwest::Client::new())
                .await
                .expect("corrupt cache is replaced");
            serde_json::from_slice::<serde_json::Value>(&std::fs::read(path).unwrap())
                .expect("repaired JSON cache");
            let repaired = cache::read_for(&cfg, &ValidatedUrl::try_new(server.uri()).unwrap(), 30)
                .expect("cache readable")
                .expect("fresh entry cached");
            assert!(repaired.headers.iter().any(|(name, value)| {
                name.as_str() == "x-session-id" && value.to_str().ok() == Some("corrupt-repair")
            }));
            assert_eq!(server.received_requests().await.expect("mints").len(), 2);
        });
    });
}

#[test]
fn unreadable_cache_fails_closed_before_contacting_the_gateway() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_with(ResponseTemplate::new(200).set_body_json(auth_response_body()))
                .expect(0)
                .mount(&server)
                .await;
            let cfg = pat_config(&home, &server.uri());
            let cache_dir = home.path().join(".cache");
            std::fs::create_dir_all(&cache_dir).unwrap();
            let expected = cache_dir.join(systemprompt_bridge::brand::brand().working_dir_name);
            std::fs::create_dir_all(expected.join("cache.json")).unwrap();
            let error = auth::read_or_refresh(
                &cfg,
                30,
                &SessionId::new("io-failure"),
                &reqwest::Client::new(),
            )
            .await
            .expect_err("cache read I/O failure must stop before minting");
            assert!(error.to_string().contains("cache.json"), "{error}");
            assert!(
                server
                    .received_requests()
                    .await
                    .expect("requests")
                    .is_empty()
            );
        });
    });
}
