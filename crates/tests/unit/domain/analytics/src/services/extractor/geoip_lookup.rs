//! GeoIP enrichment behavior of `SessionAnalytics`, driven through the public
//! `from_headers_and_uri` seam against the MaxMind test database.
//!
//! The fixture is `GeoIP2-City-Test.mmdb` from the MaxMind-DB test corpus;
//! 89.160.20.128 is its canonical Swedish (SE, Linköping) test address.
//! The client IP is now injected already-resolved via `caller_ip`; the
//! extractor no longer parses it from headers.

use std::net::IpAddr;
use std::sync::Arc;

use axum::http::HeaderMap;
use systemprompt_analytics::{GeoIpReader, SessionAnalytics};

fn test_reader() -> GeoIpReader {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/GeoIP2-City-Test.mmdb"
    );
    Arc::new(maxminddb::Reader::open_readfile(path).expect("open MaxMind test database"))
}

fn reader_from(fixture: &str) -> GeoIpReader {
    let path = format!("{}/fixtures/{fixture}", env!("CARGO_MANIFEST_DIR"));
    Arc::new(maxminddb::Reader::open_readfile(&path).expect("open MaxMind test database"))
}

fn debug_subscriber_guard() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .finish();
    tracing::subscriber::set_default(subscriber)
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

#[test]
fn public_ip_in_the_database_is_enriched_with_country_region_and_city() {
    let reader = test_reader();
    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_geoip(&reader)
        .with_caller_ip(ip("89.160.20.128"))
        .build();

    assert_eq!(analytics.country.as_deref(), Some("SE"));
    assert_eq!(analytics.region.as_deref(), Some("E"));
    assert_eq!(analytics.city.as_deref(), Some("Linköping"));
}

#[test]
fn ip_absent_from_the_database_leaves_geo_fields_empty() {
    let reader = test_reader();
    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_geoip(&reader)
        .with_caller_ip(ip("203.0.113.7"))
        .build();

    assert_eq!(analytics.country, None);
    assert_eq!(analytics.region, None);
    assert_eq!(analytics.city, None);
}

#[test]
fn loopback_unspecified_private_and_link_local_addresses_are_never_looked_up() {
    let reader = test_reader();
    for addr in [
        "127.0.0.1",
        "0.0.0.0",
        "10.1.2.3",
        "192.168.1.1",
        "169.254.10.10",
    ] {
        let analytics = SessionAnalytics::builder(&HeaderMap::new())
            .with_geoip(&reader)
            .with_caller_ip(ip(addr))
            .build();
        assert_eq!(analytics.country, None, "{addr} must not be geolocated");
    }
}

#[test]
fn tree_traversal_error_from_a_corrupt_database_yields_no_geo() {
    // A database with broken tree pointers makes `reader.lookup` fail during
    // traversal; the enrichment must swallow the error and return no geo. A
    // DEBUG subscriber is active so the failure-log fields are evaluated.
    let _guard = debug_subscriber_guard();
    let reader = reader_from("MaxMind-DB-test-broken-pointers-24.mmdb");
    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_geoip(&reader)
        .with_caller_ip(ip("1.1.1.32"))
        .build();
    assert_eq!(analytics.country, None);
    assert_eq!(analytics.region, None);
    assert_eq!(analytics.city, None);
}

#[test]
fn record_decode_error_from_a_corrupt_database_yields_no_geo() {
    // This database resolves the record but its city payload carries a
    // malformed double, so `.decode()` fails; enrichment must still degrade to
    // no geo rather than propagate the error.
    let _guard = debug_subscriber_guard();
    let reader = reader_from("GeoIP2-City-Test-Broken-Double-Format.mmdb");
    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_geoip(&reader)
        .with_caller_ip(ip("89.160.20.128"))
        .build();
    assert_eq!(analytics.country, None);
    assert_eq!(analytics.region, None);
    assert_eq!(analytics.city, None);
}

#[test]
fn missing_client_ip_and_missing_reader_skip_enrichment() {
    let reader = test_reader();
    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_geoip(&reader)
        .build();
    assert_eq!(analytics.country, None);

    let analytics = SessionAnalytics::builder(&HeaderMap::new())
        .with_caller_ip(ip("89.160.20.128"))
        .build();
    assert_eq!(analytics.country, None);
}
