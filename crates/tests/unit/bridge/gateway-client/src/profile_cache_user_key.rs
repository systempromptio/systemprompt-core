//! The bridge profile is fetched as a user and cached per `(gateway, user)`.
//!
//! Two users of one gateway get separate entries; a 401 on a stale bearer
//! re-mints exactly once; a second 401 (or a 401 on a fresh bearer) is an
//! error, and no request ever goes out without a bearer.

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use systemprompt_bridge::auth::profile::{ProfileFetchError, fetch_as_user};
use systemprompt_bridge::auth::profile_cache::load_for;
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::profile_fetch::{AgedBearer, is_unauthorized};
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::{UserId, ValidatedUrl};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ALICE: &str = "h.eyJzdWIiOiAidXNlci1hbGljZSJ9.s";
const BOB: &str = "h.eyJzdWIiOiAidXNlci1ib2IifQ.s";
const STALE: &str = "h.eyJzdWIiOiAidXNlci1hbGljZS1zdGFsZSJ9.s";
const FRESH: &str = "h.eyJzdWIiOiAidXNlci1hbGljZS1mcmVzaCJ9.s";

fn profile_body(model: &str) -> serde_json::Value {
    serde_json::json!({
        "inference_gateway_base_url": "https://gw.example.com",
        "auth_scheme": "bearer",
        "models": [model],
        "organization_uuid": "f8e4d915-f8ad-5304-ab0d-c1bf895df963"
    })
}

fn sandbox_vars(base: &Path, gateway_uri: &str) -> Vec<(&'static str, Option<OsString>)> {
    let dirs = ["config", "cache", "data", "state", "home"].map(|d| base.join(d));
    for d in &dirs {
        fs::create_dir_all(d).unwrap();
    }
    let config_file = dirs[0].join("systemprompt-bridge.toml");
    fs::write(&config_file, format!("gateway_url = \"{gateway_uri}\"\n")).unwrap();
    vec![
        ("SP_BRIDGE_CONFIG", Some(config_file.into())),
        ("XDG_CONFIG_HOME", Some(dirs[0].clone().into())),
        ("XDG_CACHE_HOME", Some(dirs[1].clone().into())),
        ("XDG_DATA_HOME", Some(dirs[2].clone().into())),
        ("XDG_STATE_HOME", Some(dirs[3].clone().into())),
        ("HOME", Some(dirs[4].clone().into())),
    ]
}

fn bearer_is(token: &str) -> wiremock::matchers::HeaderExactMatcher {
    header("authorization", format!("Bearer {token}").as_str())
}

fn in_sandbox<T>(f: impl FnOnce(&tokio::runtime::Runtime, &MockServer, ValidatedUrl) -> T) -> T {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());
    let temp = tempfile::tempdir().unwrap();
    let vars = sandbox_vars(temp.path(), &server.uri());
    let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
    temp_env::with_vars(vars, || f(&rt, &server, gateway))
}

fn user(id: &str) -> UserId {
    UserId::try_new(id).unwrap()
}

#[test]
fn two_users_of_one_gateway_get_separate_entries() {
    in_sandbox(|rt, server, gateway| {
        rt.block_on(async {
            for (token, model) in [(ALICE, "alice-model"), (BOB, "bob-model")] {
                Mock::given(method("GET"))
                    .and(path("/v1/bridge/profile"))
                    .and(bearer_is(token))
                    .respond_with(ResponseTemplate::new(200).set_body_json(profile_body(model)))
                    .mount(server)
                    .await;
            }
            let client = GatewayClient::new(gateway.clone(), reqwest::Client::new());
            for token in [ALICE, BOB] {
                fetch_as_user(
                    &client,
                    AgedBearer::fresh(BearerToken::new(token)),
                    || async { unreachable!("a served bearer is not re-minted") },
                )
                .await
                .unwrap();
            }
        });
        let alice = load_for(gateway.as_str(), &user("user-alice"))
            .unwrap()
            .unwrap();
        let bob = load_for(gateway.as_str(), &user("user-bob"))
            .unwrap()
            .unwrap();
        assert_eq!(alice.models, vec!["alice-model".to_owned()]);
        assert_eq!(bob.models, vec!["bob-model".to_owned()]);
        assert!(
            load_for(gateway.as_str(), &user("user-carol"))
                .unwrap()
                .is_none(),
            "a user the gateway never served has no entry"
        );
        assert!(
            load_for("https://other.example.com", &user("user-alice"))
                .unwrap()
                .is_none(),
            "another gateway's entry is not this one's"
        );
    });
}

fn counting_remint(
    calls: &Arc<AtomicUsize>,
) -> impl FnOnce() -> std::future::Ready<Result<BearerToken, ProfileFetchError>> + Send {
    let calls = Arc::clone(calls);
    move || {
        calls.fetch_add(1, Ordering::SeqCst);
        std::future::ready(Ok(BearerToken::new(FRESH)))
    }
}

fn stale(token: &str) -> AgedBearer {
    AgedBearer {
        token: BearerToken::new(token),
        age: Duration::from_secs(10 * 60),
    }
}

#[test]
fn stale_bearer_401_remints_once_then_succeeds() {
    in_sandbox(|rt, server, gateway| {
        let calls = Arc::new(AtomicUsize::new(0));
        rt.block_on(async {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/profile"))
                .and(bearer_is(STALE))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(server)
                .await;
            Mock::given(method("GET"))
                .and(path("/v1/bridge/profile"))
                .and(bearer_is(FRESH))
                .respond_with(ResponseTemplate::new(200).set_body_json(profile_body("m")))
                .expect(1)
                .mount(server)
                .await;
            let client = GatewayClient::new(gateway.clone(), reqwest::Client::new());
            let profile = fetch_as_user(&client, stale(STALE), counting_remint(&calls))
                .await
                .unwrap();
            assert_eq!(profile.models, vec!["m".to_owned()]);
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            load_for(gateway.as_str(), &user("user-alice-fresh"))
                .unwrap()
                .is_some(),
            "the profile is cached under the re-minted bearer's user"
        );
    });
}

#[test]
fn second_401_is_an_error_with_no_anonymous_fallback() {
    in_sandbox(|rt, server, gateway| {
        let calls = Arc::new(AtomicUsize::new(0));
        rt.block_on(async {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/profile"))
                .respond_with(ResponseTemplate::new(401))
                .mount(server)
                .await;
            let client = GatewayClient::new(gateway.clone(), reqwest::Client::new());
            let err = fetch_as_user(&client, stale(STALE), counting_remint(&calls))
                .await
                .unwrap_err();
            assert!(
                matches!(&err, ProfileFetchError::Gateway(e) if is_unauthorized(e)),
                "got {err}"
            );
            let requests = server.received_requests().await.unwrap();
            assert_eq!(
                requests.len(),
                2,
                "the stale bearer, then the re-minted one"
            );
            assert!(
                requests
                    .iter()
                    .all(|r| r.headers.get("authorization").is_some()),
                "no request goes out without a bearer"
            );
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(
            load_for(gateway.as_str(), &user("user-alice-stale"))
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn fresh_bearer_401_is_not_reminted() {
    in_sandbox(|rt, server, gateway| {
        let calls = Arc::new(AtomicUsize::new(0));
        rt.block_on(async {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/profile"))
                .respond_with(ResponseTemplate::new(401))
                .expect(1)
                .mount(server)
                .await;
            let client = GatewayClient::new(gateway, reqwest::Client::new());
            let err = fetch_as_user(
                &client,
                AgedBearer::fresh(BearerToken::new(ALICE)),
                counting_remint(&calls),
            )
            .await
            .unwrap_err();
            assert!(matches!(&err, ProfileFetchError::Gateway(e) if is_unauthorized(e)));
        });
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn a_bearer_naming_no_user_is_served_but_not_cached() {
    in_sandbox(|rt, server, gateway| {
        rt.block_on(async {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/profile"))
                .respond_with(ResponseTemplate::new(200).set_body_json(profile_body("m")))
                .mount(server)
                .await;
            let client = GatewayClient::new(gateway.clone(), reqwest::Client::new());
            fetch_as_user(
                &client,
                AgedBearer::fresh(BearerToken::new("opaque-token")),
                || async { unreachable!("a served bearer is not re-minted") },
            )
            .await
            .unwrap();
        });
        let meta = systemprompt_bridge::config::paths::bridge_metadata_dir().unwrap();
        assert!(!meta.join("bridge-profile.json").exists());
    });
}
