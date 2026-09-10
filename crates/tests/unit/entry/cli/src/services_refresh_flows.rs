//! `core services refresh --check` against a stubbed bundle origin.
//!
//! The check arm decides whether a supervisor is told to restart, so what it
//! compares (the cached digest against the origin's ETag) and how it reports
//! an origin it cannot read are exercised through the command itself.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_cli::cli_settings::{CliConfig, OutputFormat};
use systemprompt_cli::context::CommandContext;
use systemprompt_cli::core::services::refresh::{RefreshArgs, execute};
use systemprompt_cli::env_overrides::EnvOverrides;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::bundle::{BundleCache, cache_root};
use systemprompt_models::services::bundle::{BundleSourceState, ServicesBundleState};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::services_profile_fixture as fx;

const ETAG: &str = "sha256:cafebabe";

fn ctx() -> CommandContext {
    CommandContext::new(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
    )
}

fn prime_state(digest: &str) {
    let profile = ProfileBootstrap::get().expect("profile installed");
    let cache = BundleCache::new(cache_root(profile));
    let mut sources = BTreeMap::new();
    sources.insert(
        "base".to_owned(),
        BundleSourceState {
            digest: digest.to_owned(),
            version: "1.2.3".to_owned(),
            content_hash: "hash".to_owned(),
            fetched_at: Utc::now(),
        },
    );
    cache
        .write_state(&ServicesBundleState {
            composed_hash: "composed".to_owned(),
            last_reconciled_hash: None,
            sources,
        })
        .expect("state written");
}

async fn origin(status: u16, etag: Option<&str>) -> MockServer {
    let server = MockServer::start().await;
    let mut response = ResponseTemplate::new(status);
    if let Some(etag) = etag {
        response = response.insert_header("etag", format!("\"{etag}\""));
    }
    Mock::given(method("HEAD"))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn install_profile(url: &str) -> fx::ProfileTree {
    let tree = fx::write_tree(
        &fx::https_sources_block(&[("base", url)]),
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    fx::set_env("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS", "127.0.0.1,localhost");
    ProfileBootstrap::init_from_path(&tree.profile_path).expect("profile installs");
    tree
}

#[tokio::test]
async fn a_matching_digest_leaves_the_composition_alone() {
    let server = origin(200, Some(ETAG)).await;
    let _tree = install_profile(&format!("{}/bundle.tar.gz", server.uri()));
    prime_state(ETAG);

    execute(&RefreshArgs { check: true }, &ctx())
        .await
        .expect("check reports no change and returns");
}

#[tokio::test]
async fn an_origin_that_refuses_the_credential_fails_the_check() {
    let server = origin(403, None).await;
    let _tree = install_profile(&format!("{}/bundle.tar.gz", server.uri()));
    prime_state(ETAG);

    let error = execute(&RefreshArgs { check: true }, &ctx())
        .await
        .expect_err("a source that cannot be read is an error");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("base"),
        "the failing source is not named: {rendered}"
    );
    assert!(
        rendered.contains("could not be reached") || rendered.contains("not usable"),
        "unexpected failure: {rendered}"
    );
}

#[tokio::test]
async fn a_profile_with_no_sources_reports_nothing_to_do() {
    let tree = fx::write_tree(
        &fx::https_sources_block(&[]),
        "secrets:\n  secrets_path: secrets.json\n  source: env\n",
    );
    ProfileBootstrap::init_from_path(&tree.profile_path).expect("profile installs");

    execute(&RefreshArgs { check: true }, &ctx())
        .await
        .expect("an empty source list is not a failure");
}
