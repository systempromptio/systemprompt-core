use systemprompt_bridge::gateway::manifest::AutoUpdatePolicy;
use systemprompt_bridge::update::{auto_update_policy, automatic_enabled, run_automatic};
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
        block_on(run_automatic(&gateway, "bearer", &http));
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
        block_on(run_automatic(&gateway, "bearer", &http));
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
        block_on(run_automatic(&gateway, "bearer", &http));
    });

    assert_eq!(
        requests(&server).len(),
        1,
        "a failed manifest read is not retried into a download"
    );
}

#[test]
fn an_unsynced_bridge_stages_updates_by_default() {
    let state = TempDir::new().expect("state");
    assert_eq!(
        in_sandbox(&state, auto_update_policy),
        AutoUpdatePolicy::Staged,
        "with no delivered policy the default stages rather than refusing"
    );
}
