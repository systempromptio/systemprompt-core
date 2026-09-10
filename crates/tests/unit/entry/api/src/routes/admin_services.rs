use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use systemprompt_api::routes::admin::services::{RefreshLock, build_status};
use systemprompt_loader::services_root::{ActiveServicesRoot, ServicesProvenance};
use systemprompt_models::services::bundle::{BundleSourceState, ServicesBundleState};

fn staged_state() -> ServicesBundleState {
    let mut sources = BTreeMap::new();
    sources.insert(
        "astound".to_owned(),
        BundleSourceState {
            digest: "sha256:aaa".to_owned(),
            version: "1.4.0".to_owned(),
            content_hash: "ch-aaa".to_owned(),
            fetched_at: Utc
                .with_ymd_and_hms(2026, 9, 10, 8, 0, 0)
                .single()
                .expect("valid timestamp"),
        },
    );
    ServicesBundleState {
        composed_hash: "composed-1".to_owned(),
        last_reconciled_hash: Some("composed-0".to_owned()),
        sources,
    }
}

#[test]
fn no_sources_reports_bundled_with_no_source_rows() {
    let active = ActiveServicesRoot {
        path: PathBuf::from("/app/services"),
        provenance: ServicesProvenance::Bundled,
    };

    let status = build_status(
        Some(&active),
        "/app/services",
        false,
        &ServicesBundleState::default(),
    );

    assert_eq!(status.provenance.kind, "bundled");
    assert_eq!(status.active_root, "/app/services");
    assert!(status.sources.is_empty());
    assert_eq!(status.provenance.composed_hash, None);
    assert_eq!(status.last_reconciled_hash, None);
}

#[test]
fn no_sources_ignores_a_stale_cache_state() {
    let status = build_status(None, "/app/services", false, &staged_state());

    assert_eq!(status.provenance.kind, "bundled");
    assert!(
        status.sources.is_empty(),
        "a profile with no sources must not report cached source rows"
    );
}

#[test]
fn a_fetched_root_reports_its_sources_and_hash() {
    let mut versions = BTreeMap::new();
    versions.insert("astound".to_owned(), "1.4.0".to_owned());
    let active = ActiveServicesRoot {
        path: PathBuf::from("/app/cache/current"),
        provenance: ServicesProvenance::Fetched {
            composed_hash: "composed-1".to_owned(),
            versions,
        },
    };

    let status = build_status(Some(&active), "/app/services", true, &staged_state());

    assert_eq!(status.provenance.kind, "fetched");
    assert_eq!(
        status.provenance.composed_hash.as_deref(),
        Some("composed-1")
    );
    assert_eq!(status.provenance.error, None);
    assert_eq!(status.active_root, "/app/cache/current");
    assert_eq!(status.last_reconciled_hash.as_deref(), Some("composed-0"));
    assert_eq!(status.sources.len(), 1);
    let source = &status.sources[0];
    assert_eq!(source.name, "astound");
    assert_eq!(source.digest, "sha256:aaa");
    assert_eq!(source.version, "1.4.0");
    assert_eq!(source.content_hash, "ch-aaa");
}

#[test]
fn a_last_good_root_carries_the_failure_text() {
    let active = ActiveServicesRoot {
        path: PathBuf::from("/app/cache/current"),
        provenance: ServicesProvenance::LastGood {
            composed_hash: "composed-1".to_owned(),
            error: "source astound: fetch failed: connection refused".to_owned(),
        },
    };

    let status = build_status(Some(&active), "/app/services", true, &staged_state());

    assert_eq!(status.provenance.kind, "last_good");
    assert_eq!(
        status.provenance.error.as_deref(),
        Some("source astound: fetch failed: connection refused")
    );
}

#[test]
fn a_bundled_fallback_reports_bundled_and_keeps_the_error() {
    let active = ActiveServicesRoot {
        path: PathBuf::from("/app/services"),
        provenance: ServicesProvenance::BundledFallback {
            error: "no cached bundle".to_owned(),
        },
    };

    let status = build_status(Some(&active), "/app/services", true, &staged_state());

    assert_eq!(status.provenance.kind, "bundled");
    assert_eq!(status.provenance.error.as_deref(), Some("no cached bundle"));
}

#[test]
fn status_serialises_with_the_documented_field_names() {
    let status = build_status(
        None,
        "/app/services",
        false,
        &ServicesBundleState::default(),
    );
    let json = serde_json::to_value(&status).expect("status serialises");

    assert!(json.get("active_root").is_some());
    assert_eq!(json["provenance"]["kind"], "bundled");
    assert_eq!(json["sources"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn a_second_refresh_cannot_take_the_single_flight_lock() {
    let lock = RefreshLock::default();
    let held = lock.try_acquire().expect("first caller takes the lock");

    assert!(
        lock.try_acquire().is_none(),
        "a concurrent refresh must be refused, not queued"
    );

    drop(held);
    assert!(
        lock.try_acquire().is_some(),
        "the lock must be released when the refresh finishes"
    );
}
