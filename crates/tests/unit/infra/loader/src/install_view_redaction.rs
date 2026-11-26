//! What an install reads of the services sources through the install view:
//! a pin's digest is compared by its content address, so a query or fragment
//! after it (where a signed URL keeps its credential) neither reaches the view
//! nor changes the digest provenance compares; and a fetch failure's text is
//! never in the view's provenance.

use systemprompt_loader::bundle::{BaseTreeHashMemo, sources_provenance};
use systemprompt_manifest::profile::FetchFailurePolicy;
use systemprompt_manifest::services::InstallServicesView;
use systemprompt_manifest::services::root::{ActiveServicesRoot, ServicesProvenance};

use crate::bundle_profile::{bundle_sources, https_source, profile};

const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn a_pins_query_is_neither_shown_nor_part_of_its_digest() {
    let root = tempfile::tempdir().expect("fixture");
    let pinned = |query: &str| {
        profile(
            &root.path().join("services"),
            &root.path().join("cache"),
            vec![https_source(
                "kit",
                &format!("https://bundles.example/kit@{DIGEST}{query}"),
                Vec::new(),
            )],
            FetchFailurePolicy::FailClosed,
        )
    };
    let signed = pinned("?X-Amz-Signature=secret#frag");
    let plain = pinned("");
    let memo = BaseTreeHashMemo::new();
    let digest = |p| {
        sources_provenance(&bundle_sources(p), None, &memo).bundles[0]
            .pinned_digest
            .clone()
    };
    assert_eq!(digest(&signed).as_deref(), Some(DIGEST));
    assert_eq!(
        digest(&signed),
        digest(&plain),
        "the digest is the pin's content address"
    );
    let view = InstallServicesView::of(
        systemprompt_test_fixtures::fixture_services_view(),
        &ActiveServicesRoot {
            path: root.path().to_path_buf(),
            base: root.path().to_path_buf(),
            provenance: ServicesProvenance::BundledFallback {
                error: "GET https://user:pw@bundles.example/kit?sig=secret failed".to_owned(),
            },
        },
        &signed,
        root.path().join("cache"),
    );
    let location = view.sources()[0].location.clone().expect("a location");
    assert!(
        !location.contains("secret") && !location.contains("frag"),
        "{location}"
    );
    let shown = format!("{:?}", view.active_root());
    assert!(
        !shown.contains("secret") && !shown.contains("bundles.example"),
        "a fetch error's text is withheld: {shown}"
    );
}

#[test]
fn a_source_with_both_transports_is_reported_under_its_oci_reference() {
    let root = tempfile::tempdir().expect("fixture");
    let mut both = https_source("kit", "https://bundles.example/kit.tar", Vec::new());
    both.oci = Some(systemprompt_manifest::profile::OciServicesSource {
        reference: "registry.example/kit:1".to_owned(),
        auth_secret: None,
        verify: systemprompt_manifest::profile::BundleVerification::default(),
    });
    let profile = profile(
        &root.path().join("services"),
        &root.path().join("cache"),
        vec![both],
        FetchFailurePolicy::FailClosed,
    );
    let view = InstallServicesView::of(
        systemprompt_test_fixtures::fixture_services_view(),
        &ActiveServicesRoot {
            path: root.path().to_path_buf(),
            base: root.path().to_path_buf(),
            provenance: ServicesProvenance::Bundled,
        },
        &profile,
        root.path().join("cache"),
    );
    assert_eq!(
        view.sources().len(),
        1,
        "the source stays in provenance reporting"
    );
    assert_eq!(
        view.sources()[0].location.as_deref(),
        Some("registry.example/kit:1")
    );
}
