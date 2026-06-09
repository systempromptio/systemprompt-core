//! `GeoIP` lookup helpers for [`super::SessionAnalytics`]. Compiled to a
//! no-op when the `geolocation` feature is disabled so callers can keep a
//! uniform signature.

use crate::GeoIpReader;

#[cfg(feature = "geolocation")]
pub(super) fn lookup_geoip(
    ip_str: &str,
    geoip_reader: Option<&GeoIpReader>,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let Some(reader) = geoip_reader else {
        tracing::debug!(ip = %ip_str, "GeoIP lookup skipped: reader not configured");
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
        tracing::debug!(ip = %ip_str, "GeoIP lookup skipped: loopback or unspecified address");
        return None;
    }

    if let std::net::IpAddr::V4(ipv4) = ip
        && (ipv4.is_private() || ipv4.is_link_local())
    {
        tracing::debug!(ip = %ip_str, "GeoIP lookup skipped: private or link-local address");
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
pub(super) const fn lookup_geoip(
    _ip_str: &str,
    _geoip_reader: Option<&GeoIpReader>,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    None
}

pub(super) fn parse_referrer_source(url: &str) -> Option<String> {
    match url::Url::parse(url) {
        Ok(parsed_url) => parsed_url
            .host_str()
            .map(str::to_owned)
            .filter(|host| host.parse::<std::net::IpAddr>().is_err()),
        Err(err) => {
            tracing::debug!(url = %url, error = %err, "failed to parse referrer URL");
            None
        },
    }
}
