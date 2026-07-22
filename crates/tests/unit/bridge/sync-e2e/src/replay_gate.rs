//! The non-forced `run_once` gates that `apply.rs` bypasses with
//! `force_replay=true`: corrupt replay-state refusal, manifest-version replay
//! rejection, `not_before` skew rejection, the missing org-plugins directory,
//! and sentinel persistence after a successful non-forced apply.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::gateway::manifest::SignedManifest;
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::ids::ManifestSignature;
use systemprompt_bridge::sync::run_once;
use systemprompt_test_fixtures::fixture_user_id;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Sandbox {
    config_file: PathBuf,
    config_home: PathBuf,
    cache_home: PathBuf,
    data_home: PathBuf,
    state_home: PathBuf,
    home: PathBuf,
    metadata: PathBuf,
    org_plugins: PathBuf,
    _temp: tempfile::TempDir,
}

fn fresh_version(now: chrono::DateTime<chrono::Utc>) -> ManifestVersion {
    let stamp = now.format("%Y-%m-%dT%H:%M:%SZ");
    ManifestVersion::try_new(format!("{stamp}-0123abcd")).unwrap()
}

fn manifest(now: chrono::DateTime<chrono::Utc>, not_before: &str) -> SignedManifest {
    SignedManifest {
        manifest_version: fresh_version(now),
        issued_at: not_before.to_owned(),
        not_before: not_before.to_owned(),
        user_id: fixture_user_id(),
        tenant_id: None,
        user: None,
        plugins: vec![],
        skills: vec![],
        agents: vec![],
        hooks: vec![],
        managed_mcp_servers: vec![],
        revocations: vec![],
        enabled_hosts: vec![],
        host_model_protocols: Default::default(),
        artifacts: vec![],
        signature: ManifestSignature::new(""),
    }
}

fn serve(m: &SignedManifest) -> (MockServer, Sandbox) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/auth/bridge/pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": "test-bearer-token",
                "ttl": 3600,
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/bridge/manifest"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::to_value(m).unwrap()),
            )
            .mount(&server)
            .await;
        let sandbox = build_sandbox(&server.uri());
        (server, sandbox)
    })
}

fn build_sandbox(gateway_uri: &str) -> Sandbox {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    let config_home = base.join("config");
    let cache_home = base.join("cache");
    let data_home = base.join("data");
    let state_home = base.join("state");
    let home = base.join("home");
    for d in [&config_home, &cache_home, &data_home, &state_home, &home] {
        fs::create_dir_all(d).unwrap();
    }
    let org_plugins = data_home.join("Claude").join("org-plugins");
    fs::create_dir_all(&org_plugins).unwrap();

    let pat_file = base.join("pat.txt");
    fs::write(&pat_file, "sp-live-test-pat").unwrap();

    let config_file = config_home.join("systemprompt-bridge.toml");
    fs::write(
        &config_file,
        format!(
            "gateway_url = \"{gateway_uri}\"\n[pat]\nfile = \"{}\"\n",
            pat_file.display()
        ),
    )
    .unwrap();

    let metadata = state_home.join("systemprompt-bridge").join("metadata");
    Sandbox {
        config_file,
        config_home,
        cache_home,
        data_home,
        state_home,
        home,
        metadata,
        org_plugins,
        _temp: temp,
    }
}

fn run_gated(sandbox: &Sandbox) -> Result<systemprompt_bridge::sync::SyncSummary, String> {
    let config_file: OsString = sandbox.config_file.clone().into();
    temp_env::with_vars(
        [
            ("SP_BRIDGE_CONFIG", Some(config_file)),
            ("XDG_CONFIG_HOME", Some(sandbox.config_home.clone().into())),
            ("XDG_CACHE_HOME", Some(sandbox.cache_home.clone().into())),
            ("XDG_DATA_HOME", Some(sandbox.data_home.clone().into())),
            ("XDG_STATE_HOME", Some(sandbox.state_home.clone().into())),
            ("HOME", Some(sandbox.home.clone().into())),
        ],
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_once(true, false, true))
                .map_err(|e| e.to_string())
        },
    )
}

fn write_sentinel(sandbox: &Sandbox, body: &str) -> PathBuf {
    fs::create_dir_all(&sandbox.metadata).unwrap();
    let path = sandbox.metadata.join("last-sync.json");
    fs::write(&path, body).unwrap();
    path
}

#[test]
fn a_corrupt_replay_sentinel_refuses_to_apply() {
    let now = chrono::Utc::now();
    let (_server, sandbox) = serve(&manifest(now, &now.to_rfc3339()));
    write_sentinel(&sandbox, "{ this is not json");

    let err = run_gated(&sandbox).expect_err("corrupt state must refuse");
    assert!(
        err.contains("replay state corrupt"),
        "error names the corrupt state: {err}"
    );
    assert!(
        err.contains("last-sync.json"),
        "error carries the sentinel path: {err}"
    );
}

#[test]
fn a_manifest_version_not_newer_than_the_last_applied_one_is_rejected() {
    let now = chrono::Utc::now();
    let m = manifest(now, &now.to_rfc3339());
    let (_server, sandbox) = serve(&m);
    write_sentinel(
        &sandbox,
        &serde_json::json!({
            "last_applied_manifest_version": m.manifest_version.to_string(),
        })
        .to_string(),
    );

    let err = run_gated(&sandbox).expect_err("a replayed version must be rejected");
    assert!(
        err.contains("manifest replay rejected"),
        "error names the replay gate: {err}"
    );
}

#[test]
fn a_not_before_outside_the_skew_window_is_rejected() {
    let now = chrono::Utc::now();
    let stale = (now - chrono::Duration::hours(2)).to_rfc3339();
    let (_server, sandbox) = serve(&manifest(now, &stale));

    let err = run_gated(&sandbox).expect_err("stale not_before must be rejected");
    assert!(
        err.contains("clock skew rejected"),
        "error names the skew gate: {err}"
    );
}

#[test]
fn a_missing_org_plugins_directory_fails_with_the_provisioning_hint() {
    let now = chrono::Utc::now();
    let (_server, sandbox) = serve(&manifest(now, &now.to_rfc3339()));
    fs::remove_dir_all(&sandbox.org_plugins).unwrap();

    let err = run_gated(&sandbox).expect_err("missing plugin dir must fail");
    assert!(
        err.contains("does not exist") && err.contains("install --apply"),
        "error carries the provisioning hint: {err}"
    );
}

fn sentinel_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("sentinel written")).unwrap()
}

#[test]
fn a_successful_non_forced_apply_persists_the_replay_sentinel() {
    let now = chrono::Utc::now();
    let m = manifest(now, &now.to_rfc3339());
    let (_server, sandbox) = serve(&m);

    let summary = run_gated(&sandbox).expect("gated sync applies");
    assert_eq!(summary.manifest_version, m.manifest_version.to_string());

    let sentinel = sentinel_json(&sandbox.metadata.join("last-sync.json"));
    assert_eq!(
        sentinel["last_applied_manifest_version"],
        m.manifest_version.to_string()
    );
    assert!(
        sentinel["last_applied_at"].as_str().is_some(),
        "sentinel records when the manifest was applied: {sentinel}"
    );

    let err = run_gated(&sandbox).expect_err("the same version is now a replay");
    assert!(err.contains("manifest replay rejected"), "{err}");
}
