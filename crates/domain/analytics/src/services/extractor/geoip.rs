//! `GeoIP` lookup helpers for [`super::SessionAnalytics`]. Compiled to a
//! no-op when the `geolocation` feature is disabled so callers can keep a
//! uniform signature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::GeoIpReader;

#[cfg(feature = "geolocation")]
pub(crate) fn lookup_geoip(
    ip_str: &str,
    geoip_reader: Option<&GeoIpReader>,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let Some(reader) = geoip_reader else {
        static NO_READER_NOTICE: std::sync::Once = std::sync::Once::new();
        NO_READER_NOTICE.call_once(|| {
            tracing::debug!(
                geoip_skip_reason = "no_reader",
                "GeoIP enrichment inactive: no MaxMind reader configured; location fields will be empty for all requests"
            );
        });
        return None;
    };

    let ip: std::net::IpAddr = match ip_str.parse() {
        Ok(ip) => ip,
        Err(e) => {
            tracing::debug!(ip = %ip_str, error = %e, "GeoIP lookup failed: invalid IP address");
            return None;
        },
    };

    if ip.is_loopback() || ip.is_unspecified() {
        tracing::trace!(ip = %ip_str, geoip_skip_reason = "private_or_local_ip", "GeoIP lookup skipped");
        return None;
    }

    if let std::net::IpAddr::V4(ipv4) = ip
        && (ipv4.is_private() || ipv4.is_link_local())
    {
        tracing::trace!(ip = %ip_str, geoip_skip_reason = "private_or_local_ip", "GeoIP lookup skipped");
        return None;
    }

    let lookup_result = match reader.lookup(ip) {
        Ok(result) => result,
        Err(e) => {
            tracing::debug!(ip = %ip_str, error = %e, "GeoIP lookup failed: database lookup error");
            return None;
        },
    };

    let city_data: maxminddb::geoip2::City = match lookup_result.decode() {
        Ok(Some(data)) => data,
        Ok(None) => {
            tracing::debug!(ip = %ip_str, "GeoIP lookup returned empty result");
            return None;
        },
        Err(e) => {
            tracing::debug!(ip = %ip_str, error = %e, "GeoIP decode failed");
            return None;
        },
    };

    let country = city_data.country.iso_code.map(str::to_owned);

    let region = city_data
        .subdivisions
        .first()
        .and_then(|s| s.iso_code)
        .map(str::to_owned);

    let city_name = city_data.city.names.english.map(str::to_owned);

    Some((country, region, city_name))
}

#[cfg(not(feature = "geolocation"))]
pub(crate) const fn lookup_geoip(
    _ip_str: &str,
    _geoip_reader: Option<&GeoIpReader>,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    None
}
