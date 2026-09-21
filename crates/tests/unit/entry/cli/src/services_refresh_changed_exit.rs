#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use chrono::Utc;
use systemprompt_cli::core::services::refresh::{EXIT_CHANGED, RefreshArgs, execute};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_models::services::bundle::{BundleSourceState, ServicesBundleState};
use systemprompt_test_fixtures::ensure_test_secrets_bootstrap;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::services_profile_fixture as fixture;

const HELPER: &str = "services_refresh_changed_exit::refresh_changed_helper";
const OLD: &str = "sha256:old00000";
const NEW: &str = "sha256:new11111";

#[tokio::test]
#[ignore = "re-executed by changed_source_reports_both_digests_and_documented_restart_exit"]
async fn refresh_changed_helper() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", format!("\"{NEW}\"")))
        .mount(&server)
        .await;
    let tree = fixture::write_tree(
        &fixture::https_sources_block(&[("base", &format!("{}/bundle.tar.gz", server.uri()))]),
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    fixture::set_env("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS", "127.0.0.1,localhost");
    ProfileBootstrap::init_from_path(&tree.profile_path).expect("install refresh profile");
    ensure_test_secrets_bootstrap();
    let profile = ProfileBootstrap::get().unwrap();
    let cache = BundleCache::new(cache_root(profile));
    let mut sources = BTreeMap::new();
    sources.insert(
        "base".to_owned(),
        BundleSourceState {
            digest: OLD.to_owned(),
            version: "1.2.3".to_owned(),
            content_hash: "old-content".to_owned(),
            fetched_at: Utc::now(),
        },
    );
    cache
        .write_state(&ServicesBundleState {
            composed_hash: "old-composition".to_owned(),
            last_reconciled_hash: None,
            sources,
        })
        .unwrap();
    let context = CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    );
    println!("BEGIN_CHANGED_REFRESH");
    execute(&RefreshArgs { check: true }, &context)
        .await
        .expect("changed check exits with its documented supervisor code");
    panic!("changed refresh returned instead of exiting");
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().unwrap();
    let stderr = tempfile::NamedTempFile::new().unwrap();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn changed-refresh helper");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().unwrap();
            panic!(
                "changed-refresh helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn changed_source_reports_both_digests_and_documented_restart_exit() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert_eq!(output.status.code(), Some(EXIT_CHANGED));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 refresh output");
    let json = stdout
        .split_once("BEGIN_CHANGED_REFRESH")
        .map(|(_, json)| json.trim())
        .unwrap_or_else(|| panic!("missing refresh marker in {stdout}"));
    let artifact: serde_json::Value = serde_json::from_str(json)
        .unwrap_or_else(|error| panic!("invalid refresh JSON: {error}: {json}"));
    assert_eq!(artifact["artifact_type"], "table", "{artifact}");
    assert_eq!(artifact["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(artifact["items"][0]["name"], "base");
    assert_eq!(artifact["items"][0]["previous_digest"], OLD);
    assert_eq!(artifact["items"][0]["new_digest"], NEW);
    assert_eq!(artifact["items"][0]["changed"], true);
}
