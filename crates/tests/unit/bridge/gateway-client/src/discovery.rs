use systemprompt_bridge::auth::providers::session::device_link_url;
use systemprompt_bridge::gateway::discovery::{
    self, DiscoveryError, DiscoveryOutcome, is_gateway_path,
};
use systemprompt_bridge::instance::DisplayBrand;
use systemprompt_identifiers::ValidatedUrl;
use systemprompt_models::ApiPaths;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sandbox(home: &TempDir) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/config"))),
        ("XDG_STATE_HOME", Some(format!("{root}/state"))),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_PAT", None),
    ]
}

fn run<F: std::future::Future>(home: &TempDir, f: impl FnOnce() -> F) -> F::Output {
    temp_env::with_vars(sandbox(home), || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(f())
    })
}

fn point_profile_at(gateway: &ValidatedUrl) {
    let profile = systemprompt_bridge::config::gateways::active().unwrap();
    std::fs::create_dir_all(&profile.dir).unwrap();
    std::fs::write(
        &profile.config_file,
        format!("gateway_url = \"{}\"\n", gateway.as_str()),
    )
    .unwrap();
}

fn document() -> serde_json::Value {
    serde_json::json!({
        "contract": "1",
        "device_link_path": "/bridge-auth/device-link",
        "identity_path": "/api/public/bridge/whoami",
        "branding": { "display_name": "Acme Bridge", "sign_in_hint": "Use your Acme account" },
        "enabled_hosts": ["codex-cli"]
    })
}

#[test]
fn a_served_document_is_cached_and_supplies_routes_and_branding() {
    let home = TempDir::new().unwrap();
    run(&home, || async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(ApiPaths::WELLKNOWN_BRIDGE))
            .respond_with(ResponseTemplate::new(200).set_body_json(document()))
            .mount(&server)
            .await;
        let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
        point_profile_at(&gateway);

        let outcome = discovery::refresh_active(&reqwest::Client::new())
            .await
            .unwrap();
        assert!(matches!(outcome, DiscoveryOutcome::Fetched(_)));

        assert_eq!(discovery::whoami_path(), "/api/public/bridge/whoami");
        assert!(
            device_link_url(gateway.as_str(), None).ends_with("/bridge-auth/device-link"),
            "the sign-in page is the instance's own"
        );
        assert_eq!(
            discovery::enabled_hosts().unwrap(),
            vec![systemprompt_bridge::integration::HostKind::CodexCli]
        );
        let display = DisplayBrand::resolve();
        assert_eq!(display.app_name, "Acme Bridge");
        assert_eq!(display.sign_in_hint, "Use your Acme account");
        assert!(
            display.theme_css_url.is_none(),
            "discovery never supplies a stylesheet"
        );
    });
}

#[test]
fn a_gateway_without_discovery_keeps_the_compiled_routes() {
    let home = TempDir::new().unwrap();
    run(&home, || async {
        let server = MockServer::start().await;
        let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
        point_profile_at(&gateway);

        let outcome = discovery::refresh_active(&reqwest::Client::new())
            .await
            .unwrap();
        assert_eq!(outcome, DiscoveryOutcome::NotServed);
        assert_eq!(discovery::whoami_path(), ApiPaths::BRIDGE_WHOAMI);
        let brand = systemprompt_bridge::brand::brand();
        assert!(device_link_url(gateway.as_str(), None).ends_with(brand.device_link_path));
        assert_eq!(DisplayBrand::resolve().app_name, brand.app_name);
    });
}

#[test]
fn an_unreachable_gateway_is_an_error_and_leaves_the_compiled_routes() {
    let home = TempDir::new().unwrap();
    run(&home, || async {
        let gateway = ValidatedUrl::try_new("http://127.0.0.1:9").unwrap();
        point_profile_at(&gateway);
        let err = discovery::refresh_active(&reqwest::Client::new())
            .await
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::Network { .. }), "{err:?}");
        assert_eq!(discovery::whoami_path(), ApiPaths::BRIDGE_WHOAMI);
    });
}

#[test]
fn a_cache_from_another_gateway_is_never_read() {
    let home = TempDir::new().unwrap();
    run(&home, || async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(ApiPaths::WELLKNOWN_BRIDGE))
            .respond_with(ResponseTemplate::new(200).set_body_json(document()))
            .mount(&server)
            .await;
        let gateway = ValidatedUrl::try_new(server.uri()).unwrap();
        point_profile_at(&gateway);
        discovery::refresh_active(&reqwest::Client::new())
            .await
            .unwrap();

        point_profile_at(&ValidatedUrl::try_new("https://other.example.com").unwrap());
        assert!(discovery::cached().is_none());
        assert_eq!(discovery::whoami_path(), ApiPaths::BRIDGE_WHOAMI);
    });
}

#[test]
fn a_document_naming_a_foreign_authority_is_refused() {
    let home = TempDir::new().unwrap();
    run(&home, || async {
        let server = MockServer::start().await;
        let mut doc = document();
        doc["identity_path"] = "@evil.example.com/whoami".into();
        Mock::given(method("GET"))
            .and(path(ApiPaths::WELLKNOWN_BRIDGE))
            .respond_with(ResponseTemplate::new(200).set_body_json(doc))
            .mount(&server)
            .await;
        point_profile_at(&ValidatedUrl::try_new(server.uri()).unwrap());
        let err = discovery::refresh_active(&reqwest::Client::new())
            .await
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::ForeignPath { .. }), "{err:?}");
        assert!(discovery::cached().is_none());
    });
}

#[test]
fn only_plain_gateway_paths_are_accepted() {
    assert!(is_gateway_path("/bridge-auth/device-link"));
    for bad in [
        "//evil.example.com/x",
        "@evil.example.com",
        "relative",
        "/a\\b",
        "/a b",
    ] {
        assert!(!is_gateway_path(bad), "{bad:?}");
    }
}
