use std::io::Read as _;
use systemprompt_bridge::ids::BearerToken;
use systemprompt_bridge::update::{
    AutoUpdateDecision, auto_update_policy, automatic_enabled, run_automatic,
};

use sha2::{Digest as _, Sha256};
use systemprompt_identifiers::ValidatedUrl;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(fut)
}

fn seed_last_sync(state: &TempDir, gateway: &ValidatedUrl, auto_update: &str) {
    let meta = state.path().join("systemprompt-bridge").join("metadata");
    std::fs::create_dir_all(&meta).expect("metadata dir");
    std::fs::write(
        meta.join("last-sync.json"),
        serde_json::json!({ "gateway": gateway, "auto_update": auto_update }).to_string(),
    )
    .expect("seed last-sync");
    let config_dir = state.path().join("systemprompt");
    std::fs::create_dir_all(&config_dir).expect("config dir");
    std::fs::write(
        config_dir.join("systemprompt-bridge.toml"),
        format!("gateway_url = \"{gateway}\"\n"),
    )
    .expect("seed config");
}

fn in_sandbox<R>(state: &TempDir, f: impl FnOnce() -> R) -> R {
    let root = state.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("HOME", Some(root.clone())),
            ("XDG_STATE_HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(root)),
        ],
        f,
    )
}

fn release_server(version: &str) -> MockServer {
    let version = version.to_owned();
    block_on(async move {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": version,
                "sha256": "0".repeat(64),
                "size": 0,
            })))
            .mount(&server)
            .await;
        server
    })
}

fn requests(server: &MockServer) -> Vec<wiremock::Request> {
    block_on(server.received_requests()).unwrap_or_default()
}

fn file_digest(path: &std::path::Path) -> [u8; 32] {
    let mut file = std::fs::File::open(path).expect("open current executable");
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).expect("read current executable");
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    hasher.finalize().into()
}

#[test]
fn a_disabled_policy_stops_the_update_before_the_gateway_is_contacted() {
    let state = TempDir::new().expect("state");
    let server = release_server("99.0.0");
    let gateway = ValidatedUrl::try_new(&server.uri()).expect("gateway url");
    seed_last_sync(&state, &gateway, "disabled");
    let http = reqwest::Client::new();

    in_sandbox(&state, || {
        assert!(
            !automatic_enabled(),
            "a delivered `disabled` policy turns updates off"
        );
        block_on(run_automatic(&gateway, &BearerToken::new("bearer"), &http));
    });

    assert!(
        requests(&server).is_empty(),
        "no release manifest is fetched once policy has refused the update"
    );
}

#[test]
fn a_gateway_with_no_newer_release_fetches_once_and_installs_nothing() {
    let state = TempDir::new().expect("state");
    let server = release_server("0.0.1");
    let gateway = ValidatedUrl::try_new(&server.uri()).expect("gateway url");
    seed_last_sync(&state, &gateway, "staged");
    let http = reqwest::Client::new();

    in_sandbox(&state, || {
        assert!(
            automatic_enabled(),
            "a delivered `staged` policy leaves updates on"
        );
        block_on(run_automatic(&gateway, &BearerToken::new("bearer"), &http));
    });

    let seen = requests(&server);
    assert_eq!(
        seen.len(),
        1,
        "the manifest is read once and no artifact download follows"
    );
    assert!(
        seen[0].url.path().ends_with("/v1/bridge/latest"),
        "the only call is the release manifest: {}",
        seen[0].url
    );
}

#[test]
fn a_gateway_that_cannot_answer_leaves_the_installed_binary_alone() {
    let state = TempDir::new().expect("state");
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/latest"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        server
    });
    let gateway = ValidatedUrl::try_new(&server.uri()).expect("gateway url");
    seed_last_sync(&state, &gateway, "staged");
    let http = reqwest::Client::new();

    in_sandbox(&state, || {
        block_on(run_automatic(&gateway, &BearerToken::new("bearer"), &http));
    });

    assert_eq!(
        requests(&server).len(),
        1,
        "a failed manifest read is not retried into a download"
    );
}

#[test]
fn an_unsynced_bridge_withholds_automatic_updates() {
    let state = TempDir::new().expect("state");
    let decision = in_sandbox(&state, auto_update_policy);
    assert!(
        matches!(decision, AutoUpdateDecision::NeverSynced),
        "with no manifest synced the policy is withheld: {}",
        decision.describe()
    );
    assert!(
        !decision.stages(),
        "an unsynced bridge never stages a download on its own"
    );
}

#[test]
fn a_staged_policy_rejects_a_bad_artifact_without_staging_or_relaunching() {
    let state = TempDir::new().expect("state");
    let server = block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": "99.0.0",
                "sha256": "0".repeat(64),
                "size": 3,
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/v1/bridge/download/{}",
                systemprompt_bridge::update::platform_slug().expect("supported test platform")
            )))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bad"))
            .expect(1)
            .mount(&server)
            .await;
        server
    });
    let gateway = ValidatedUrl::try_new(&server.uri()).expect("gateway url");
    seed_last_sync(&state, &gateway, "staged");
    let executable = std::env::current_exe().expect("running executable");
    let before = file_digest(&executable);
    let staging = in_sandbox(&state, || {
        systemprompt_bridge::config::paths::bridge_update_dir().expect("owned update dir resolves")
    });

    in_sandbox(&state, || {
        block_on(run_automatic(
            &gateway,
            &BearerToken::new("bearer"),
            &reqwest::Client::new(),
        ));
    });

    assert_eq!(
        requests(&server).len(),
        2,
        "one manifest and one artifact request"
    );
    let actual = file_digest(&executable);
    assert!(
        actual == before,
        "a failed digest never replaces or launches over the running binary"
    );
    assert!(
        !staging.exists()
            || std::fs::read_dir(&staging)
                .expect("owned staging")
                .next()
                .is_none(),
        "a mismatched artifact leaves no staged executable"
    );
}
