//! GeoIP enrichment behavior of `SessionAnalytics`, driven through the public
//! `from_headers_with_geoip` seam against the MaxMind test database.
//!
//! The fixture is `GeoIP2-City-Test.mmdb` from the MaxMind-DB test corpus;
//! 89.160.20.128 is its canonical Swedish (SE, Linköping) test address.

use std::sync::Arc;

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::{GeoIpReader, SessionAnalytics};

fn test_reader() -> GeoIpReader {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/GeoIP2-City-Test.mmdb"
    );
    Arc::new(maxminddb::Reader::open_readfile(path).expect("open MaxMind test database"))
}

fn headers_with_ip(ip: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_str(ip).unwrap());
    headers
}

#[test]
fn public_ip_in_the_database_is_enriched_with_country_region_and_city() {
    let reader = test_reader();
    let analytics =
        SessionAnalytics::from_headers_with_geoip(&headers_with_ip("89.160.20.128"), Some(&reader));

    assert_eq!(analytics.country.as_deref(), Some("SE"));
    assert_eq!(analytics.region.as_deref(), Some("E"));
    assert_eq!(analytics.city.as_deref(), Some("Linköping"));
}

#[test]
fn ip_absent_from_the_database_leaves_geo_fields_empty() {
    let reader = test_reader();
    let analytics =
        SessionAnalytics::from_headers_with_geoip(&headers_with_ip("203.0.113.7"), Some(&reader));

    assert_eq!(analytics.country, None);
    assert_eq!(analytics.region, None);
    assert_eq!(analytics.city, None);
}

#[test]
fn loopback_unspecified_private_and_link_local_addresses_are_never_looked_up() {
    let reader = test_reader();
    for ip in [
        "127.0.0.1",
        "0.0.0.0",
        "10.1.2.3",
        "192.168.1.1",
        "169.254.10.10",
    ] {
        let analytics =
            SessionAnalytics::from_headers_with_geoip(&headers_with_ip(ip), Some(&reader));
        assert_eq!(analytics.country, None, "{ip} must not be geolocated");
    }
}

#[test]
fn unparseable_ip_and_missing_reader_skip_enrichment() {
    let reader = test_reader();
    let analytics =
        SessionAnalytics::from_headers_with_geoip(&headers_with_ip("not-an-ip"), Some(&reader));
    assert_eq!(analytics.country, None);

    let analytics =
        SessionAnalytics::from_headers_with_geoip(&headers_with_ip("89.160.20.128"), None);
    assert_eq!(analytics.country, None);
}
