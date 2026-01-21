use axum::extract::Request;
use axum::http::{HeaderMap, Uri};
use std::collections::HashMap;

use super::bot_keywords::{matches_bot_ip_range, matches_bot_pattern};
use super::detection;
use super::user_agent::parse_user_agent;
use crate::GeoIpReader;

#[derive(Debug, Clone)]
pub struct SessionAnalytics {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_type: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub preferred_locale: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub referrer_source: Option<String>,
    pub referrer_url: Option<String>,
    pub landing_page: Option<String>,
    pub entry_url: Option<String>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
}

impl SessionAnalytics {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        Self::from_headers_with_geoip(headers, None)
    }

    pub fn from_headers_with_geoip(
        headers: &HeaderMap,
        geoip_reader: Option<&GeoIpReader>,
    ) -> Self {
        Self::from_headers_with_geoip_and_socket(headers, geoip_reader, None)
    }

    pub fn from_headers_with_geoip_and_socket(
        headers: &HeaderMap,
        geoip_reader: Option<&GeoIpReader>,
        socket_addr: Option<std::net::SocketAddr>,
    ) -> Self {
        let user_agent = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);

        let ip_address = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().to_string())
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|v| v.to_str().ok())
                    .map(ToString::to_string)
            })
            .or_else(|| socket_addr.map(|addr| addr.ip().to_string()));

        let fingerprint_hash = headers
            .get("x-fingerprint")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);

        let preferred_locale = headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().split(';').next().unwrap_or(s).to_string());

        let (device_type, browser, os) = user_agent
            .as_ref()
            .map_or((None, None, None), |ua| parse_user_agent(ua));

        let (country, region, city) = ip_address
            .as_ref()
            .and_then(|ip_str| Self::lookup_geoip(ip_str, geoip_reader))
            .unwrap_or((None, None, None));

        let referrer_url = headers
            .get("referer")
            .and_then(|v| v.to_str().ok())
            .map(ToString::to_string);

        let referrer_source = referrer_url
            .as_ref()
            .and_then(|url| Self::parse_referrer_source(url));

        Self {
            ip_address,
            user_agent,
            device_type,
            browser,
            os,
            fingerprint_hash,
            preferred_locale,
            country,
            region,
            city,
            referrer_source,
            referrer_url,
            landing_page: None,
            entry_url: None,
            utm_source: None,
            utm_medium: None,
            utm_campaign: None,
        }
    }

    pub fn from_headers_and_uri(
        headers: &HeaderMap,
        uri: Option<&Uri>,
        geoip_reader: Option<&GeoIpReader>,
        content_routing: Option<&dyn systemprompt_models::ContentRouting>,
    ) -> Self {
        let mut analytics = Self::from_headers_with_geoip(headers, geoip_reader);

        if let Some(uri) = uri {
            let query_params = Self::parse_query_params(uri);

            analytics.utm_source = query_params.get("utm_source").cloned();
            analytics.utm_medium = query_params.get("utm_medium").cloned();
            analytics.utm_campaign = query_params.get("utm_campaign").cloned();

            let is_html_page =
                content_routing.is_some_and(|routing| routing.is_html_page(uri.path()));

            if is_html_page {
                analytics.entry_url = Some(uri.to_string());
                analytics.landing_page = Some(uri.path().to_string());
            }
        }

        analytics
    }

    pub fn from_request(
        request: &Request,
        geoip_reader: Option<&GeoIpReader>,
        content_routing: Option<&dyn systemprompt_models::ContentRouting>,
    ) -> Self {
        Self::from_headers_and_uri(
            request.headers(),
            Some(request.uri()),
            geoip_reader,
            content_routing,
        )
    }

    fn parse_query_params(uri: &Uri) -> HashMap<String, String> {
        uri.query().map_or_else(HashMap::new, |q| {
            q.split('&')
                .filter_map(|param| {
                    let mut parts = param.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect()
        })
    }

    fn lookup_geoip(
        ip_str: &str,
        geoip_reader: Option<&GeoIpReader>,
    ) -> Option<(Option<String>, Option<String>, Option<String>)> {
        let reader = geoip_reader?;
        let ip: std::net::IpAddr = ip_str.parse().ok()?;

        let city_data: maxminddb::geoip2::City = reader.lookup(ip).ok()?;

        let country = city_data
            .country
            .and_then(|c| c.iso_code)
            .map(ToString::to_string);

        let region = city_data
            .subdivisions
            .and_then(|subdivisions| subdivisions.first().cloned())
            .and_then(|s| s.iso_code)
            .map(ToString::to_string);

        let city_name = city_data
            .city
            .and_then(|c| c.names)
            .and_then(|names| names.get("en").copied())
            .map(ToString::to_string);

        Some((country, region, city_name))
    }

    fn parse_referrer_source(url: &str) -> Option<String> {
        url::Url::parse(url)
            .ok()
            .and_then(|parsed_url| parsed_url.host_str().map(ToString::to_string))
            .and_then(|host| {
                if host.parse::<std::net::IpAddr>().is_ok() {
                    None
                } else {
                    Some(host)
                }
            })
    }

    pub fn is_bot(&self) -> bool {
        self.user_agent
            .as_ref()
            .is_some_and(|ua| matches_bot_pattern(ua))
    }

    pub fn is_bot_ip(&self) -> bool {
        self.ip_address
            .as_ref()
            .is_some_and(|ip| matches_bot_ip_range(ip))
    }

    pub fn is_spam_referrer(&self) -> bool {
        self.referrer_url
            .as_ref()
            .is_some_and(|url| detection::is_spam_referrer(url))
    }

    pub fn is_datacenter_ip(&self) -> bool {
        self.ip_address
            .as_ref()
            .is_some_and(|ip| detection::is_datacenter_ip(ip))
    }

    pub fn is_high_risk_country(&self) -> bool {
        self.country
            .as_ref()
            .is_some_and(|c| detection::is_high_risk_country(c))
    }

    pub fn should_skip_tracking(&self) -> bool {
        self.is_bot()
            || self.is_bot_ip()
            || self.is_datacenter_ip()
            || self.is_high_risk_country()
            || self.is_spam_referrer()
    }
}
