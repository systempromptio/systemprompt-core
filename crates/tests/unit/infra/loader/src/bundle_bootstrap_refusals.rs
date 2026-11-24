//! One refused source must not hold the others back, and must not pass
//! unnoticed.
//!
//! A kit whose newest bundle this build cannot load keeps its cached bundle
//! while the rest of the composition advances; the refusal names the source,
//! the reason and the upload that was refused, and the next good resolve
//! clears it. `fail_closed` still refuses outright.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_loader::bundle::{BundleCache, RefusalKind, ServicesSourceBootstrap};
use systemprompt_manifest::profile::{FetchFailurePolicy, ServicesSource};
use systemprompt_manifest::services::root::ServicesProvenance;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::bundle_profile::{https_source, profile};
use crate::bundle_support::{base_tree, marketplace_tree, pack, pubkey};

const CORE: &str = "0.82.0";

fn no_secrets(_name: &str) -> Option<String> {
    None
}

fn packed(build: impl Fn(&Path), dir: &Path, name: &str, version: &str, requires: &str) -> Vec<u8> {
    let tree = tempfile::tempdir().expect("tempdir");
    build(tree.path());
    let archive = dir.join(name);
    pack(tree.path(), &archive, version, requires);
    std::fs::read(&archive).expect("read archive")
}

async fn serve(bytes: Vec<u8>, etag: &str, route: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(route.to_owned()))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path(route.to_owned()))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", etag))
        .mount(&server)
        .await;
    server
}

fn source(name: &str, server: &MockServer, route: &str) -> ServicesSource {
    https_source(name, &format!("{}{route}", server.uri()), vec![pubkey()])
}

fn kit_tree(root: &Path) {
    marketplace_tree(root, "uk", "sales");
}

fn kit_with_a_second_plugin(root: &Path) {
    marketplace_tree(root, "uk", "sales");
    marketplace_tree(root, "uk", "support");
}

#[tokio::test]
async fn a_kit_requiring_a_newer_core_is_held_back_while_the_base_advances() {
    let work = tempfile::tempdir().expect("tempdir");
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let base = serve(
        packed(base_tree, work.path(), "base.tar.gz", "1.0.0", ">=0.1"),
        "base-v1",
        "/base.tar.gz",
    )
    .await;
    let kit = serve(
        packed(kit_tree, work.path(), "kit.tar.gz", "4.2.0", ">=0.82"),
        "kit-v1",
        "/kit.tar.gz",
    )
    .await;
    let first = profile(
        services.path(),
        cache.path(),
        vec![
            source("base", &base, "/base.tar.gz"),
            source("kit", &kit, "/kit.tar.gz"),
        ],
        FetchFailurePolicy::UseLastGood,
    );
    ServicesSourceBootstrap::resolve(&first, no_secrets, CORE)
        .await
        .expect("first boot");

    let newer_kit = serve(
        packed(
            kit_with_a_second_plugin,
            work.path(),
            "kit-next.tar.gz",
            "4.3.0",
            ">=0.83",
        ),
        "kit-v2",
        "/kit.tar.gz",
    )
    .await;
    let second = profile(
        services.path(),
        cache.path(),
        vec![
            source("base", &base, "/base.tar.gz"),
            source("kit", &newer_kit, "/kit.tar.gz"),
        ],
        FetchFailurePolicy::UseLastGood,
    );
    let after = ServicesSourceBootstrap::resolve(&second, no_secrets, CORE)
        .await
        .expect("a refused kit must not stop the resolve");

    assert!(
        matches!(after.provenance, ServicesProvenance::Fetched { .. }),
        "the composition loads, so it is not a fallback: {:?}",
        after.provenance
    );
    assert!(
        !after.path.join("plugins/support/config.yaml").exists(),
        "nothing from the refused bundle is served"
    );
    let state = BundleCache::new(cache.path()).read_state();
    assert_eq!(state.sources["kit"].version, "4.2.0");

    let refusals = BundleCache::new(cache.path())
        .read_refusals()
        .expect("refusals");
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    let refusal = &refusals[0];
    assert_eq!(refusal.source, "kit");
    assert_eq!(refusal.kind, RefusalKind::RequiresCore);
    assert!(
        refusal.reason.contains("requires core >=0.83") && refusal.reason.contains("0.82.0"),
        "the reason names both versions: {}",
        refusal.reason
    );
    assert_eq!(refusal.attempted_digest.as_deref(), Some("kit-v2"));
    assert_eq!(refusal.serving_version, "4.2.0");
    assert_eq!(refusal.serving_digest, "kit-v1");
}

#[tokio::test]
async fn the_refusal_is_cleared_by_the_next_bundle_that_loads() {
    let work = tempfile::tempdir().expect("tempdir");
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let base = serve(
        packed(base_tree, work.path(), "base.tar.gz", "1.0.0", ">=0.1"),
        "base-v1",
        "/base.tar.gz",
    )
    .await;
    let boot = |kit: &MockServer| {
        profile(
            services.path(),
            cache.path(),
            vec![
                source("base", &base, "/base.tar.gz"),
                source("kit", kit, "/kit.tar.gz"),
            ],
            FetchFailurePolicy::UseLastGood,
        )
    };
    let good = serve(
        packed(kit_tree, work.path(), "kit.tar.gz", "4.2.0", ">=0.82"),
        "kit-v1",
        "/kit.tar.gz",
    )
    .await;
    ServicesSourceBootstrap::resolve(&boot(&good), no_secrets, CORE)
        .await
        .expect("first boot");
    let refused = serve(
        packed(kit_tree, work.path(), "kit-next.tar.gz", "4.3.0", ">=0.83"),
        "kit-v2",
        "/kit.tar.gz",
    )
    .await;
    ServicesSourceBootstrap::resolve(&boot(&refused), no_secrets, CORE)
        .await
        .expect("held back");
    assert_eq!(
        BundleCache::new(cache.path())
            .read_refusals()
            .expect("refusals")
            .len(),
        1
    );

    let repacked = serve(
        packed(
            kit_with_a_second_plugin,
            work.path(),
            "kit-repacked.tar.gz",
            "4.3.1",
            ">=0.82",
        ),
        "kit-v3",
        "/kit.tar.gz",
    )
    .await;
    let after = ServicesSourceBootstrap::resolve(&boot(&repacked), no_secrets, CORE)
        .await
        .expect("the repacked kit loads");

    assert!(after.path.join("plugins/support/config.yaml").is_file());
    assert!(
        BundleCache::new(cache.path())
            .read_refusals()
            .expect("refusals")
            .is_empty(),
        "a resolve that holds nothing back leaves no record"
    );
    assert_eq!(
        BundleCache::new(cache.path()).read_state().sources["kit"].version,
        "4.3.1"
    );
}

#[tokio::test]
async fn fail_closed_refuses_a_kit_this_build_cannot_load() {
    let work = tempfile::tempdir().expect("tempdir");
    let services = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let base = serve(
        packed(base_tree, work.path(), "base.tar.gz", "1.0.0", ">=0.1"),
        "base-v1",
        "/base.tar.gz",
    )
    .await;
    let kit = serve(
        packed(kit_tree, work.path(), "kit.tar.gz", "4.3.0", ">=0.83"),
        "kit-v1",
        "/kit.tar.gz",
    )
    .await;
    let p = profile(
        services.path(),
        cache.path(),
        vec![
            source("base", &base, "/base.tar.gz"),
            source("kit", &kit, "/kit.tar.gz"),
        ],
        FetchFailurePolicy::FailClosed,
    );

    let err = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect_err("fail_closed must not hold a source back");

    assert!(err.to_string().contains("fail_closed"), "{err}");
    assert!(
        BundleCache::new(cache.path())
            .read_refusals()
            .expect("refusals")
            .is_empty()
    );
}

#[tokio::test]
async fn a_source_with_nothing_cached_falls_back_whole() {
    let services = tempfile::tempdir().expect("tempdir");
    crate::bundle_support::base_tree(services.path());
    let work = tempfile::tempdir().expect("tempdir");
    let cache = tempfile::tempdir().expect("tempdir");
    let kit = serve(
        packed(kit_tree, work.path(), "kit.tar.gz", "4.3.0", ">=0.83"),
        "kit-v1",
        "/kit.tar.gz",
    )
    .await;
    let p = profile(
        services.path(),
        cache.path(),
        vec![source("kit", &kit, "/kit.tar.gz")],
        FetchFailurePolicy::UseLastGood,
    );

    let after = ServicesSourceBootstrap::resolve(&p, no_secrets, CORE)
        .await
        .expect("the baked tree still boots");

    match after.provenance {
        ServicesProvenance::BundledFallback { error } => {
            assert!(error.contains("requires core >=0.83"), "{error}");
        },
        other => panic!("expected the bundled fallback, got {other:?}"),
    }
    assert!(
        BundleCache::new(cache.path())
            .read_refusals()
            .expect("refusals")
            .is_empty()
    );
}
