//! `fetch_bridge_profile` presents the bridge's cached credential.
//!
//! The gateway lists a member's personal-key providers only for the member the
//! bearer names, so the profile fetch must send the cached token when one is
//! valid for this gateway, and still succeed anonymously when none is.

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use systemprompt_bridge::auth::cache::{CredentialBinding, write_bound};
use systemprompt_bridge::gateway::GatewayClient;
use systemprompt_bridge::gateway::types::HelperOutput;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_identifiers::ValidatedUrl;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CACHED_TOKEN: &str = "cached-member-token";

fn profile_body() -> serde_json::Value {
    serde_json::json!({
        "inference_gateway_base_url": "https://gw.example.com",
        "auth_scheme": "bearer",
        "models": ["opencode-go/model"]
    })
}

fn sandbox_vars(base: &Path, gateway_uri: &str) -> Vec<(&'static str, Option<OsString>)> {
    let dirs = ["config", "cache", "data", "state", "home"].map(|d| base.join(d));
    for d in &dirs {
        fs::create_dir_all(d).unwrap();
    }
    let pat_file = base.join("pat.txt");
    fs::write(&pat_file, "sp-live-test-pat").unwrap();
    let config_file = dirs[0].join("systemprompt-bridge.toml");
    fs::write(
        &config_file,
        format!(
            "gateway_url = \"{gateway_uri}\"\n[pat]\nfile = '{}'\n",
            pat_file.display()
        ),
    )
    .unwrap();
    vec![
        ("SP_BRIDGE_CONFIG", Some(config_file.into())),
        ("XDG_CONFIG_HOME", Some(dirs[0].clone().into())),
        ("XDG_CACHE_HOME", Some(dirs[1].clone().into())),
        ("XDG_DATA_HOME", Some(dirs[2].clone().into())),
        ("XDG_STATE_HOME", Some(dirs[3].clone().into())),
        ("HOME", Some(dirs[4].clone().into())),
    ]
}

fn seed_cache(gateway: &ValidatedUrl) {
    let cfg = systemprompt_bridge::config::load().unwrap();
    let binding = CredentialBinding::capture(&cfg).unwrap();
    write_bound(
        &cfg,
        gateway,
        &HelperOutput {
            token: BearerToken::new(CACHED_TOKEN),
            ttl: 3600,
            headers: std::collections::HashMap::new(),
        },
        &binding,
    )
    .unwrap();
}

fn run_profile_fetch(seed: bool) -> Vec<Request> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());
    let temp = tempfile::tempdir().unwrap();
    let vars = sandbox_vars(temp.path(), &server.uri());
    let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
    temp_env::with_vars(vars, || {
        if seed {
            seed_cache(&gateway);
        }
        rt.block_on(async {
            let mut mock = Mock::given(method("GET")).and(path("/v1/bridge/profile"));
            if seed {
                mock = mock.and(header(
                    "authorization",
                    format!("Bearer {CACHED_TOKEN}").as_str(),
                ));
            }
            mock.respond_with(ResponseTemplate::new(200).set_body_json(profile_body()))
                .expect(1)
                .mount(&server)
                .await;
            let client = GatewayClient::new(gateway.clone(), reqwest::Client::new());
            let profile = client.fetch_bridge_profile().await.unwrap();
            assert_eq!(profile.models, vec!["opencode-go/model".to_owned()]);
            server.received_requests().await.unwrap()
        })
    })
}

#[test]
fn fetch_bridge_profile_sends_cached_bearer() {
    let requests = run_profile_fetch(true);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].headers.get("authorization").unwrap(),
        format!("Bearer {CACHED_TOKEN}").as_str()
    );
}

#[test]
fn fetch_bridge_profile_without_cached_credential_is_anonymous() {
    let requests = run_profile_fetch(false);
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].headers.get("authorization").is_none(),
        "no cached credential means no Authorization header"
    );
}
