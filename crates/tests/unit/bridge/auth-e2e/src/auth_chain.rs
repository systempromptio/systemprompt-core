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
fn acquire_bearer_refuses_to_cache_when_the_on_disk_credential_differs() {
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
            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new(server.uri()).unwrap()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let err = auth::acquire_bearer(&cfg, &SessionId::generate(), &reqwest::Client::new())
                .await
                .expect_err(
                    "an in-memory credential the on-disk config does not know is not cached",
                );
            match err {
                ChainError::Cache(e) => assert!(
                    e.to_string().contains("no credential identity configured"),
                    "{e}"
                ),
                other => panic!("expected ChainError::Cache, got {other:?}"),
            }
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
            match err {
                ChainError::Providers(failures) => assert_eq!(
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
            match err {
                ChainError::Providers(failures) => {
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
