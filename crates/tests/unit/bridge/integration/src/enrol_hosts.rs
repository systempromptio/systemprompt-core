//! Enrolling named hosts through a real `BridgeContext`, with no gateway
//! reachable — so every local host takes the failure arm and the
//! classification arms above it are what is under test.

use std::fs;

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::integration::enrol::{Outcome, Selection, enrol_hosts};
use systemprompt_bridge::integration::reapply::ModelProtocolOverrides;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn in_sandbox<R>(f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home");
    let root = home.path().display().to_string();
    let out = temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
            ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
            ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
            ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
            ("SUDO_USER", None),
        ],
        f,
    );
    drop(home);
    out
}

fn context() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("attach context")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn enrolling_an_empty_selection_reports_nothing_and_is_not_an_error() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(Vec::new()),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect("an empty selection is valid");
        assert!(reports.is_empty());
    });
}

#[test]
fn an_unknown_host_id_fails_the_whole_request_before_any_host_is_touched() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let err = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec!["not-a-host".to_owned()]),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect_err("an unknown id is rejected outright");
        assert!(err.contains("not-a-host"), "got {err}");
        assert!(err.contains("known ids:"), "got {err}");
    });
}

#[test]
fn a_host_the_instance_does_not_enable_is_skipped_rather_than_attempted() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("an empty enabled list is a valid instance state");

        assert!(!reports.is_empty(), "there is at least one local host");
        for report in &reports {
            assert!(
                matches!(report.outcome, Outcome::NotEnabled),
                "with nothing enabled every local host is skipped, got {:?} for {}",
                report.outcome,
                report.host_id
            );
            assert!(!report.is_failure(), "a skipped host is not a failure");
        }
    });
}

#[test]
fn a_named_host_cannot_bypass_the_enabled_host_gate() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let all = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("baseline");
        let host_id = all
            .first()
            .expect("the sandbox registers at least one bridge host")
            .host_id
            .clone();

        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec![host_id.clone()]),
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("a disabled named host is a valid request");

        assert_eq!(reports.len(), 1, "only the named host is considered");
        assert_eq!(reports[0].host_id, host_id);
        assert!(
            matches!(reports[0].outcome, Outcome::NotEnabled),
            "the enabled-host filter must run before any profile write: {:?}",
            reports[0].outcome
        );
        assert!(!reports[0].is_failure());
    });
}

#[test]
fn an_enabled_list_naming_a_host_lets_that_one_through_while_skipping_the_rest() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let all = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("baseline");
        // skip-ok: no enrolled bridge host on this machine
        let Some(first) = all.first().map(|r| r.host_id.clone()) else {
            return;
        };

        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(vec![first.clone()]),
            ))
            .expect("enrol with one host enabled");

        let named = reports
            .iter()
            .find(|r| r.host_id == first)
            .expect("the named host is reported");
        assert!(
            !matches!(named.outcome, Outcome::NotEnabled),
            "the enabled host must be attempted, not skipped"
        );

        for other in reports.iter().filter(|r| r.host_id != first) {
            assert!(
                matches!(other.outcome, Outcome::NotEnabled | Outcome::SyncOnly),
                "every host not on the enabled list is skipped, got {:?} for {}",
                other.outcome,
                other.host_id
            );
        }
    });
}

#[test]
fn with_no_gateway_reachable_an_enabled_host_reports_a_failure_that_names_a_cause() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let all = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("baseline");
        // skip-ok: no enrolled bridge host on this machine
        let Some(first) = all.first().map(|r| r.host_id.clone()) else {
            return;
        };

        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec![first.clone()]),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect("the request itself is understood");

        let report = reports.first().expect("one host, one report");
        assert_eq!(report.host_id, first);
        match &report.outcome {
            Outcome::Failed(message) => assert!(
                !message.is_empty(),
                "a failure must carry the cause, not an empty string"
            ),
            other => panic!("expected a failure with no gateway configured, got {other:?}"),
        }
        assert!(report.is_failure());
    });
}

#[test]
fn a_report_is_produced_for_every_host_that_was_selected_and_no_others() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("enrol all");

        let mut ids: Vec<&str> = reports.iter().map(|r| r.host_id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "each host is reported exactly once");

        for report in &reports {
            assert!(!report.display_name.is_empty());
            assert!(!report.install_action_label.is_empty());
        }
    });
}

#[cfg(not(target_os = "macos"))]
#[test]
fn enrolling_codex_through_the_public_host_workflow_writes_a_usable_managed_provider() {
    let rt = runtime();
    let server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/profile"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "inference_gateway_base_url": server.uri(),
                "auth_scheme": "bearer",
                "models": ["gpt-5"],
                "organization_uuid": "org-enrol-test",
                "providers": [{
                    "name": "openai-upstream",
                    "surface": "openai",
                    "configured": true,
                    "models": ["gpt-5"],
                }],
            })))
            .expect(1)
            .mount(&server)
            .await;
        server
    });
    let home = TempDir::new().expect("home");
    let config_home = home.path().join("config");
    let state_home = home.path().join("state");
    let data_home = home.path().join("data");
    let cache_home = home.path().join("cache");
    let codex_home = home.path().join("codex");
    for dir in [
        &config_home,
        &state_home,
        &data_home,
        &cache_home,
        &codex_home,
    ] {
        fs::create_dir_all(dir).expect("sandbox directory");
    }
    let bridge_config = config_home.join("systemprompt-bridge.toml");
    fs::write(
        &bridge_config,
        format!("gateway_url = {:?}\n", server.uri()),
    )
    .expect("bridge config");
    let secret_dir = config_home.join("systemprompt");
    fs::create_dir_all(&secret_dir).expect("loopback secret directory");
    fs::write(
        secret_dir.join("bridge-loopback.key"),
        "enrolment-loopback-secret",
    )
    .expect("loopback secret");
    let managed = home.path().join("managed").join("config.toml");
    fs::create_dir_all(managed.parent().expect("managed parent")).expect("managed parent");
    fs::write(&managed, "operator_key = \"retain\"\n").expect("foreign managed setting");

    let reports = temp_env::with_vars(
        [
            ("HOME", Some(home.path())),
            ("XDG_CONFIG_HOME", Some(config_home.as_path())),
            ("XDG_STATE_HOME", Some(state_home.as_path())),
            ("XDG_DATA_HOME", Some(data_home.as_path())),
            ("XDG_CACHE_HOME", Some(cache_home.as_path())),
            ("SP_BRIDGE_CONFIG", Some(bridge_config.as_path())),
            ("CODEX_HOME", Some(codex_home.as_path())),
            ("CODEX_SYSTEM_CONFIG", Some(managed.as_path())),
            ("SP_BRIDGE_PAT", None),
            ("SUDO_USER", None),
        ],
        || {
            let ctx = context();
            rt.block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec!["codex-cli".to_owned()]),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect("Codex is a supported host")
        },
    );

    assert!(
        matches!(reports.as_slice(), [report] if matches!(report.outcome, Outcome::Installed)),
        "the public workflow probes the generated managed profile as installed: {reports:?}"
    );
    let managed: toml::Value =
        toml::from_str(&fs::read_to_string(&managed).expect("Codex managed config written"))
            .expect("managed config remains TOML");
    assert_eq!(managed["operator_key"].as_str(), Some("retain"));
    assert_eq!(managed["model_provider"].as_str(), Some("systemprompt"));
    assert_eq!(
        managed["model_providers"]["systemprompt"]["base_url"].as_str(),
        Some("http://127.0.0.1:48217/v1")
    );
    assert_eq!(
        managed["model_providers"]["systemprompt"]["auth"]["args"],
        toml::Value::Array(vec![
            toml::Value::String("credential-helper".to_owned()),
            toml::Value::String("--host".to_owned()),
            toml::Value::String("codex-cli".to_owned()),
        ]),
        "the generated provider routes credentials through the host-scoped helper"
    );
    assert_eq!(
        managed["model_providers"]["systemprompt"]["http_headers"]["x-tenant"].as_str(),
        Some("org-enrol-test")
    );
    assert_eq!(
        managed["model_providers"]["systemprompt"]["models"],
        toml::Value::Array(vec![toml::Value::String("gpt-5".to_owned())])
    );
    let requests = rt
        .block_on(server.received_requests())
        .expect("mock request recording");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/bridge/profile");
}
