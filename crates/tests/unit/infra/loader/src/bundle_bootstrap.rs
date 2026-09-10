//! The boot path: fetch, compose, and what happens when a refresh fails.
//!
//! The provenance is the assertion that matters. An instance that quietly
//! serves last-good content looks identical to a healthy one from the
//! outside, so every fallback must carry the error that caused it.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use systemprompt_loader::ServicesProvenance;
use systemprompt_loader::bundle::ServicesSourceBootstrap;
use systemprompt_models::profile::FetchFailurePolicy;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::bundle_profile::{https_source, profile};
use crate::bundle_support::{base_tree, marketplace_tree, pack, pubkey};

const CORE: &str = "0.49.0";

fn no_secrets(_name: &str) -> Option<String> {
    None
}

struct Origin {
    server: MockServer,
    hits: Arc<AtomicUsize>,
}

async fn serve(bytes: Vec<u8>, etag: &str, route: &str) -> Origin {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    Mock::given(method("GET"))
        .and(path(route.to_owned()))
        .respond_with(move |_req: &wiremock::Request| {
            counter.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_bytes(bytes.clone())
        })
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path(route.to_owned()))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", etag))
        .mount(&server)
        .await;
    Origin { server, hits }
}

fn packed_bytes(build: impl Fn(&Path), dir: &Path, name: &str) -> Vec<u8> {
    let tree = tempfile::tempdir().expect("tempdir");
    build(tree.path());
    let archive = dir.join(name);
    pack(tree.path(), &archive, "1.0.0", ">=0.1");
    std::fs::read(&archive).expect("read archive")
}

#[tokio::test]
async fn an_empty_source_list_serves_the_baked_tree() {
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let p = profile(
        services.path(),
        cache.path(),
        vec![],
        FetchFailurePolicy::UseLastGood,
    );

    let active = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("resolve");

    assert_eq!(active.path, services.path());
    assert_eq!(active.provenance, ServicesProvenance::Bundled);
}

#[tokio::test]
async fn two_sources_are_fetched_composed_and_then_reused_unchanged() {
    let work = tempfile::tempdir().expect("tempdir");
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let base = serve(
        packed_bytes(base_tree, work.path(), "base.tar.gz"),
        "base-v1",
        "/base.tar.gz",
    )
    .await;
    let uk = serve(
        packed_bytes(
            |root| marketplace_tree(root, "uk", "sales"),
            work.path(),
            "uk.tar.gz",
        ),
        "uk-v1",
        "/uk.tar.gz",
    )
    .await;
    let p = profile(
        services.path(),
        cache.path(),
        vec![
            https_source(
                "base",
                &format!("{}/base.tar.gz", base.server.uri()),
                vec![pubkey()],
            ),
            https_source(
                "uk",
                &format!("{}/uk.tar.gz", uk.server.uri()),
                vec![pubkey()],
            ),
        ],
        FetchFailurePolicy::UseLastGood,
    );

    let first = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("first boot");
    let second = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("second boot");

    assert!(
        first.path.join("marketplaces/uk/config.yaml").is_file(),
        "the composed root carries both bundles"
    );
    assert_eq!(
        first.provenance, second.provenance,
        "nothing changed upstream"
    );
    assert!(matches!(
        first.provenance,
        ServicesProvenance::Fetched { .. }
    ));
    assert_eq!(
        (
            base.hits.load(Ordering::SeqCst),
            uk.hits.load(Ordering::SeqCst)
        ),
        (1, 1),
        "an unchanged ETag must not re-download"
    );
}

#[tokio::test]
async fn a_failing_refresh_falls_back_to_the_last_good_composition() {
    let work = tempfile::tempdir().expect("tempdir");
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let origin = serve(
        packed_bytes(base_tree, work.path(), "base.tar.gz"),
        "base-v1",
        "/base.tar.gz",
    )
    .await;
    let url = format!("{}/base.tar.gz", origin.server.uri());
    let p = profile(
        services.path(),
        cache.path(),
        vec![https_source("base", &url, vec![pubkey()])],
        FetchFailurePolicy::UseLastGood,
    );
    let good = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("first boot");

    drop(origin);
    let broken = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/base.tar.gz"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&broken)
        .await;
    Mock::given(method("GET"))
        .and(path("/base.tar.gz"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&broken)
        .await;
    let degraded = profile(
        services.path(),
        cache.path(),
        vec![https_source(
            "base",
            &format!("{}/base.tar.gz", broken.uri()),
            vec![pubkey()],
        )],
        FetchFailurePolicy::UseLastGood,
    );
    let after = ServicesSourceBootstrap::resolve(&degraded, no_secrets, CORE)
        .await
        .expect("an unavailable origin must not stop the boot");

    match after.provenance {
        ServicesProvenance::LastGood {
            composed_hash,
            error,
        } => {
            assert!(!error.is_empty(), "the fallback records why it happened");
            assert!(matches!(
                good.provenance,
                ServicesProvenance::Fetched { composed_hash: ref h, .. } if *h == composed_hash
            ));
        },
        other => panic!("expected last-good, got {other:?}"),
    }
    assert!(after.path.join("config/config.yaml").is_file());
}

#[tokio::test]
async fn fail_closed_refuses_to_boot_on_a_failed_fetch() {
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let server = MockServer::start().await;
    let url = format!("{}/missing.tar.gz", server.uri());
    let p = profile(
        services.path(),
        cache.path(),
        vec![https_source("base", &url, vec![pubkey()])],
        FetchFailurePolicy::FailClosed,
    );

    let err = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect_err("fail_closed must not fall back");

    assert!(
        err.to_string().contains("fail_closed"),
        "the refusal names the policy: {err}"
    );
}

#[tokio::test]
async fn use_bundled_falls_back_to_the_baked_tree_when_it_exists() {
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    base_tree(services.path());
    let server = MockServer::start().await;
    let url = format!("{}/missing.tar.gz", server.uri());
    let p = profile(
        services.path(),
        cache.path(),
        vec![https_source("base", &url, vec![pubkey()])],
        FetchFailurePolicy::UseBundled,
    );

    let active = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("the baked tree is the fallback");

    assert_eq!(active.path, services.path());
    assert!(matches!(
        active.provenance,
        ServicesProvenance::BundledFallback { .. }
    ));
}
