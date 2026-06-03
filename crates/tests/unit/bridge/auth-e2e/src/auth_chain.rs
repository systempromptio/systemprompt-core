use std::io::Write;

use systemprompt_bridge::auth::{self, ChainError};
use systemprompt_bridge::config::{Config, PatConfig, SessionConfig};
use systemprompt_identifiers::{SessionId, ValidatedUrl};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a fresh tempdir-rooted env so `dirs::*`, cache reads, and config
/// resolution never touch the developer's real home. `SP_BRIDGE_PAT` and
/// `SP_BRIDGE_CONFIG` are cleared so only the in-test `Config` drives
/// behaviour.
fn sandbox_vars(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
        ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_GATEWAY_URL", None),
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
                enabled: Some(true),
            }),
            ..Config::default()
        };
        assert!(auth::has_credential_source(&cfg));
    });
}

#[test]
fn pat_provider_happy_path_yields_bearer() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_to(ResponseTemplate::new(200).set_body_json(auth_response_body()))
                .mount(&server)
                .await;

            let pat_path = home.path().join("pat.txt");
            let mut f = std::fs::File::create(&pat_path).unwrap();
            writeln!(f, "sp-live-secret-pat").unwrap();

            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new(server.uri()).unwrap()),
                pat: Some(PatConfig {
                    file: Some(pat_path.to_string_lossy().into_owned()),
                }),
                ..Config::default()
            };

            let out = auth::acquire_bearer(&cfg, &SessionId::generate())
                .await
                .expect("PAT exchange should succeed");
            assert_eq!(out.token.expose(), "sp-bearer-deadbeef-token-value");
            assert_eq!(out.ttl, 3600);
        });
    });
}

#[test]
fn pat_exchange_http_failure_yields_none_succeeded() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_to(ResponseTemplate::new(401))
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

            let err = auth::acquire_bearer(&cfg, &SessionId::generate())
                .await
                .expect_err("401 must not yield a bearer");
            assert!(
                matches!(err, ChainError::NoneSucceeded),
                "expected NoneSucceeded, got {err:?}"
            );
        });
    });
}

#[test]
fn pat_exchange_server_error_yields_none_succeeded() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/v1/auth/bridge/pat"))
                .respond_to(ResponseTemplate::new(500))
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

            let err = auth::acquire_bearer(&cfg, &SessionId::generate())
                .await
                .expect_err("500 must not yield a bearer");
            assert!(matches!(err, ChainError::NoneSucceeded));
        });
    });
}

#[test]
fn no_credential_source_yields_none_succeeded() {
    let home = TempDir::new().unwrap();
    temp_env::with_vars(sandbox_vars(&home), || {
        block_on(async {
            let cfg = Config {
                gateway_url: Some(ValidatedUrl::try_new("http://127.0.0.1:1").unwrap()),
                ..Config::default()
            };
            let err = auth::acquire_bearer(&cfg, &SessionId::generate())
                .await
                .expect_err("no provider configured must fail");
            assert!(matches!(err, ChainError::NoneSucceeded));
        });
    });
}
