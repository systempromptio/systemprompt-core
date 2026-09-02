//! Network-dependent `cloud doctor --distributed` checks: the write-primary
//! and replica-lag database probes, the `/readyz` HTTP probe, and the shape of
//! the aggregate `run` report.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::doctor::CheckStatus;
use systemprompt_cli::cloud::doctor::distributed::{
    check_readyz, check_replica_lag, check_write_primary, run,
};
use systemprompt_models::Profile;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fixture_profile() -> Profile {
    let boot = systemprompt_test_fixtures::ensure_test_bootstrap();
    let yaml = std::fs::read_to_string(&boot.profile_path).expect("read fixture profile");
    serde_yaml::from_str(&yaml).expect("parse fixture profile")
}

fn secrets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

fn live_db_url() -> String {
    systemprompt_test_fixtures::fixture_database_url()
        .expect("DATABASE_URL must be set for the distributed doctor probes")
}

// Why: a database name that does not exist on the live server fails
// permanently on the first attempt, where a refused TCP connect is classified
// retryable and spends ~3s in backoff before reporting the same warning.
fn unreachable_db_url() -> String {
    let live = live_db_url();
    let base = live.rsplit_once('/').expect("database url has a path").0;
    format!("{base}/sp_doctor_probe_absent_db")
}

#[tokio::test]
async fn write_primary_fails_when_the_write_url_is_missing() {
    let result = check_write_primary(&secrets(&[])).await;
    assert_eq!(result.status, CheckStatus::Fail, "{}", result.detail);
    assert!(
        result.detail.contains("database_write_url is not set"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn write_primary_treats_an_empty_write_url_as_unset() {
    let result = check_write_primary(&secrets(&[("database_write_url", "")])).await;
    assert_eq!(
        result.status,
        CheckStatus::Fail,
        "an empty secret is not a configured primary: {}",
        result.detail
    );
}

#[tokio::test]
async fn write_primary_warns_rather_than_fails_when_the_primary_is_unreachable() {
    let url = unreachable_db_url();
    let result = check_write_primary(&secrets(&[("database_write_url", &url)])).await;
    assert_eq!(
        result.status,
        CheckStatus::Warn,
        "an off-host run must not be reported as a misconfiguration: {}",
        result.detail
    );
    assert!(
        result.detail.contains("could not connect"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn write_primary_passes_against_a_live_primary() {
    let url = live_db_url();
    let result = check_write_primary(&secrets(&[("database_write_url", &url)])).await;
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
    assert!(result.detail.contains("is a primary"), "{}", result.detail);
}

#[tokio::test]
async fn replica_lag_fails_without_a_read_url() {
    let result = check_replica_lag(&secrets(&[("database_write_url", "postgres://x/y")])).await;
    assert_eq!(result.status, CheckStatus::Fail, "{}", result.detail);
    assert!(
        result.detail.contains("database_url is not set"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn replica_lag_skips_the_probe_when_reads_and_writes_share_one_url() {
    let url = live_db_url();
    let result = check_replica_lag(&secrets(&[
        ("database_url", &url),
        ("database_write_url", &url),
    ]))
    .await;
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
    assert!(
        result.detail.contains("no replica reads configured"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn replica_lag_skips_the_probe_when_no_write_url_is_configured() {
    let url = live_db_url();
    let result = check_replica_lag(&secrets(&[("database_url", &url)])).await;
    assert_eq!(
        result.status,
        CheckStatus::Pass,
        "a single-url deployment has no replica to lag: {}",
        result.detail
    );
    assert!(
        result.detail.contains("no replica reads configured"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn replica_lag_warns_when_the_read_url_is_unreachable() {
    let read = unreachable_db_url();
    let write = live_db_url();
    let result = check_replica_lag(&secrets(&[
        ("database_url", &read),
        ("database_write_url", &write),
    ]))
    .await;
    assert_eq!(result.status, CheckStatus::Warn, "{}", result.detail);
    assert!(
        result.detail.contains("could not connect"),
        "{}",
        result.detail
    );
}

#[tokio::test]
async fn replica_lag_warns_when_the_declared_read_replica_is_not_a_standby() {
    let read = live_db_url();
    let write = format!("{read}?application_name=primary");
    let result = check_replica_lag(&secrets(&[
        ("database_url", &read),
        ("database_write_url", &write),
    ]))
    .await;
    assert_eq!(result.status, CheckStatus::Warn, "{}", result.detail);
    assert!(
        result.detail.contains("is not a standby"),
        "a read url that differs from the write url but is writable is a \
         misconfiguration, not a healthy replica: {}",
        result.detail
    );
}

#[tokio::test]
async fn readyz_passes_when_the_probe_answers_success() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let mut profile = fixture_profile();
    profile.server.api_internal_url = server.uri();

    let result = check_readyz(&profile).await;
    assert_eq!(result.status, CheckStatus::Pass, "{}", result.detail);
    assert!(result.detail.contains("200"), "{}", result.detail);
}

#[tokio::test]
async fn readyz_warns_and_quotes_the_body_when_the_probe_reports_not_ready() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(ResponseTemplate::new(503).set_body_string("write pool unavailable"))
        .mount(&server)
        .await;

    let mut profile = fixture_profile();
    profile.server.api_internal_url = server.uri();

    let result = check_readyz(&profile).await;
    assert_eq!(result.status, CheckStatus::Warn, "{}", result.detail);
    assert!(result.detail.contains("503"), "{}", result.detail);
    assert!(
        result.detail.contains("write pool unavailable"),
        "the operator needs the reason the node is not ready: {}",
        result.detail
    );
}

#[tokio::test]
async fn readyz_trims_a_trailing_slash_off_the_configured_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut profile = fixture_profile();
    profile.server.api_internal_url = format!("{}/", server.uri());

    let result = check_readyz(&profile).await;
    assert_eq!(
        result.status,
        CheckStatus::Pass,
        "an un-trimmed slash would request //readyz and miss the route: {}",
        result.detail
    );
    assert!(!result.detail.contains("//readyz"), "{}", result.detail);
}

#[tokio::test]
async fn readyz_warns_when_the_endpoint_is_unreachable() {
    let mut profile = fixture_profile();
    profile.server.api_internal_url = "http://127.0.0.1:1".to_owned();

    let result = check_readyz(&profile).await;
    assert_eq!(result.status, CheckStatus::Warn, "{}", result.detail);
    assert!(result.detail.contains("unreachable"), "{}", result.detail);
}

#[tokio::test]
async fn run_reports_every_distributed_check_exactly_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/readyz"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let url = live_db_url();
    let mut profile = fixture_profile();
    profile.server.api_internal_url = server.uri();
    profile.server.instance_id = Some("node-a".to_owned());
    profile.server.trusted_proxies = vec!["fc00::/7".parse().expect("cidr")];

    let results = run(
        &profile,
        &secrets(&[
            ("oauth_at_rest_pepper", "pepper"),
            ("manifest_signing_secret_seed", "seed"),
            ("signing_key_pem", "pem"),
            ("database_url", &url),
            ("database_write_url", &url),
        ]),
    )
    .await;

    let names: Vec<&str> = results.iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        vec![
            "identity-fingerprints",
            "instance-id",
            "trusted-proxies",
            "write-primary",
            "replica-lag",
            "readyz",
        ]
    );
    assert!(
        results.iter().all(|r| r.status != CheckStatus::Fail),
        "a fully configured node must not fail any distributed check: {:?}",
        results
            .iter()
            .filter(|r| r.status == CheckStatus::Fail)
            .map(|r| format!("{}: {}", r.name, r.detail))
            .collect::<Vec<_>>()
    );
}
