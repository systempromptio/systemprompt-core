use systemprompt_bridge::cli::doctor::{self, Status};
use systemprompt_bridge::validate::{self, CheckLevel};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Sandbox `dirs::*` and bridge config resolution into a throwaway home so the
/// report/check code never reads the developer's real environment. The gateway
/// URL is supplied per-test via `SP_BRIDGE_GATEWAY_URL`.
fn sandbox_vars(home: &TempDir, gateway: &str) -> Vec<(&'static str, Option<String>)> {
    let root = home.path().to_string_lossy().into_owned();
    vec![
        ("HOME", Some(root.clone())),
        ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
        ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
        ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
        ("SP_BRIDGE_GATEWAY_URL", Some(gateway.to_owned())),
    ]
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

/// One mounted GET route: `path` answered with `status`. Started on its own
/// short-lived runtime; the returned `MockServer` keeps serving on its internal
/// runtime afterwards, so callers may later drive bridge code under a fresh
/// nested runtime without colliding with an outer one.
fn health_server(health_status: u16, with_whoami: bool) -> (MockServer, String) {
    block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(health_status))
            .mount(&server)
            .await;
        if with_whoami {
            Mock::given(method("GET"))
                .and(path("/v1/bridge/whoami"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
                .mount(&server)
                .await;
        }
        let uri = server.uri();
        (server, uri)
    })
}

#[test]
fn validate_run_reports_healthy_gateway() {
    let home = TempDir::new().unwrap();
    let (_server, uri) = health_server(200, false);

    temp_env::with_vars(sandbox_vars(&home, &uri), || {
        let report = block_on(validate::run());
        assert!(!report.lines.is_empty(), "report must have lines");

        let rendered = report.rendered();
        assert!(
            rendered.contains("gateway_url"),
            "rendered report should mention gateway_url:\n{rendered}"
        );
        assert!(
            rendered.contains("gateway /health"),
            "rendered report should mention gateway /health:\n{rendered}"
        );

        let health = report
            .lines
            .iter()
            .find(|l| l.label == "gateway /health")
            .expect("gateway /health line present");
        assert_eq!(health.level, CheckLevel::Ok);
    });
}

#[test]
fn validate_run_reports_failing_gateway() {
    let home = TempDir::new().unwrap();
    let (_server, uri) = health_server(503, false);

    temp_env::with_vars(sandbox_vars(&home, &uri), || {
        let report = block_on(validate::run());

        let health = report
            .lines
            .iter()
            .find(|l| l.label == "gateway /health")
            .expect("gateway /health line present");
        assert_eq!(
            health.level,
            CheckLevel::Fail,
            "503 health must produce a failing line"
        );
        assert!(
            report.any_failed,
            "a failing gateway check must set any_failed"
        );
    });
}

#[test]
fn doctor_run_checks_returns_named_checks() {
    let home = TempDir::new().unwrap();
    let (_server, uri) = health_server(200, true);

    temp_env::with_vars(sandbox_vars(&home, &uri), || {
        let (checks, any_fail) = block_on(doctor::run_checks());
        assert!(!checks.is_empty(), "doctor must emit checks");

        let names: Vec<&str> = checks.iter().map(|c| c.name).collect();
        for expected in [
            "config file",
            "credential source",
            "mint JWT",
            "gateway reachable",
            "authenticated whoami",
            "loopback secret",
            "manifest pubkey pinned",
            "hook token mint",
        ] {
            assert!(
                names.contains(&expected),
                "missing doctor check `{expected}`; got {names:?}"
            );
        }

        for c in &checks {
            assert!(matches!(c.status, Status::Ok | Status::Warn | Status::Fail));
        }
        let gateway = checks
            .iter()
            .find(|c| c.name == "gateway reachable")
            .expect("gateway reachable check present");
        assert_eq!(gateway.status, Status::Ok);

        assert!(
            any_fail,
            "no credential source should make doctor report a failure"
        );
    });
}
