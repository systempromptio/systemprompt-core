//! HTTP request → [`SessionAnalytics`] extraction.
//!
//! `HeaderValue::to_str().ok()` is used liberally below. That is deliberate:
//! a non-ASCII header value is not actionable for the analytics pipeline and
//! must not abort session creation. Treating those headers as absent is the
//! correct fallback — the corresponding session field stays `None` and
//! downstream consumers treat the request as un-attributed for that
//! dimension.
//!
//! The client IP is never parsed from hop headers here: it arrives already
//! resolved (via the HTTP boundary's trusted-proxy walk) as `caller_ip`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use http::{HeaderMap, Uri};
use std::collections::HashMap;
use std::net::IpAddr;

use super::ai_crawler_keywords::matches_ai_crawler;
use super::bot_keywords::{matches_bot_ip_range, matches_bot_pattern};
use super::detection;
use super::user_agent::parse_user_agent;
use crate::GeoIpReader;

mod geoip;

pub use systemprompt_traits::SessionAnalytics;

fn parse_query_params(uri: &Uri) -> HashMap<String, String> {
    uri.query().map_or_else(HashMap::new, |q| {
        q.split('&')
            .filter_map(|param| {
                let mut parts = param.splitn(2, '=');
                Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
            })
            .collect()
    })
}

fn parse_referrer_source(url: &str) -> Option<String> {
    match url::Url::parse(url) {
        Ok(parsed_url) => parsed_url
            .host_str()
            .map(str::to_owned)
            .filter(|host| host.parse::<IpAddr>().is_err()),
        Err(err) => {
            tracing::debug!(url = %url, error = %err, "failed to parse referrer URL");
            None
        },
    }
}

/// `headers` is the only required input; every enrichment source (URI for
/// UTM/landing-page, `GeoIP` reader, content-routing classifier, resolved
/// caller IP) is opt-in via a `with_*` setter.
pub struct SessionAnalyticsBuilder<'a> {
    headers: &'a HeaderMap,
    uri: Option<&'a Uri>,
    geoip_reader: Option<&'a GeoIpReader>,
    content_routing: Option<&'a dyn systemprompt_models::ContentRouting>,
    caller_ip: Option<IpAddr>,
}

impl std::fmt::Debug for SessionAnalyticsBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAnalyticsBuilder")
            .field("headers", &self.headers)
            .field("uri", &self.uri)
            .field("geoip_reader", &self.geoip_reader.map(|_| "<reader>"))
            .field(
                "content_routing",
                &self.content_routing.map(|_| "<content_routing>"),
            )
            .field("caller_ip", &self.caller_ip)
            .finish()
    }
}

impl<'a> SessionAnalyticsBuilder<'a> {
    #[must_use]
    pub const fn new(headers: &'a HeaderMap) -> Self {
        Self {
            headers,
            uri: None,
            geoip_reader: None,
            content_routing: None,
            caller_ip: None,
        }
    }

    #[must_use]
    pub const fn with_uri(mut self, uri: &'a Uri) -> Self {
        self.uri = Some(uri);
        self
    }

    #[must_use]
    pub const fn with_geoip(mut self, reader: &'a GeoIpReader) -> Self {
        self.geoip_reader = Some(reader);
        self
    }

    #[must_use]
    pub fn with_content_routing(
        mut self,
        content_routing: &'a dyn systemprompt_models::ContentRouting,
    ) -> Self {
        self.content_routing = Some(content_routing);
        self
    }

    #[must_use]
    pub const fn with_caller_ip(mut self, caller_ip: IpAddr) -> Self {
        self.caller_ip = Some(caller_ip);
        self
    }

    #[must_use]
    pub fn build(self) -> SessionAnalytics {
        let headers = self.headers;

        let user_agent = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let ip_address = self.caller_ip.map(|ip| ip.to_string());

        let fingerprint_hash = headers
            .get("x-fingerprint")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let preferred_locale = headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().split(';').next().unwrap_or(s).to_owned());

        let (device_type, browser, os) = user_agent
            .as_ref()
            .map_or((None, None, None), |ua| parse_user_agent(ua));

        let (country, region, city) = ip_address
            .as_ref()
            .and_then(|ip_str| geoip::lookup_geoip(ip_str, self.geoip_reader))
            .unwrap_or((None, None, None));

        let referrer_url = headers
            .get("referer")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let referrer_source = referrer_url.as_deref().and_then(parse_referrer_source);

        let mut analytics = SessionAnalytics {
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
            utm_content: None,
            utm_term: None,
            is_bot: false,
            is_ai_crawler: false,
            skip_tracking: false,
        };

        if let Some(uri) = self.uri {
            Self::apply_uri(&mut analytics, uri, self.content_routing);
        }

        Self::classify(&mut analytics);

        analytics
    }

    /// Decides the bot/crawler verdicts once, at the only place that owns the
    /// keyword tables. Must run after `apply_uri` — `skip_tracking` consults
    /// the referrer, and the country comes from the `GeoIP` lookup above.
    fn classify(analytics: &mut SessionAnalytics) {
        analytics.is_ai_crawler = analytics
            .user_agent
            .as_ref()
            .is_some_and(|ua| matches_ai_crawler(ua));

        analytics.is_bot = !analytics.is_ai_crawler
            && analytics
                .user_agent
                .as_ref()
                .is_none_or(|ua| ua.is_empty() || matches_bot_pattern(ua));

        let flagged_origin = analytics
            .ip_address
            .as_ref()
            .is_some_and(|ip| matches_bot_ip_range(ip) || detection::is_datacenter_ip(ip))
            || analytics
                .country
                .as_ref()
                .is_some_and(|c| detection::is_high_risk_country(c))
            || analytics
                .referrer_url
                .as_ref()
                .is_some_and(|url| detection::is_spam_referrer(url));

        analytics.skip_tracking = !analytics.is_ai_crawler && (analytics.is_bot || flagged_origin);
    }

    fn apply_uri(
        analytics: &mut SessionAnalytics,
        uri: &Uri,
        content_routing: Option<&dyn systemprompt_models::ContentRouting>,
    ) {
        let query_params = parse_query_params(uri);

        analytics.utm_source = query_params.get("utm_source").cloned();
        analytics.utm_medium = query_params.get("utm_medium").cloned();
        analytics.utm_campaign = query_params.get("utm_campaign").cloned();
        analytics.utm_content = query_params.get("utm_content").cloned();
        analytics.utm_term = query_params.get("utm_term").cloned();

        if content_routing.is_some_and(|routing| routing.is_html_page(uri.path())) {
            analytics.entry_url = Some(uri.to_string());
            analytics.landing_page = Some(uri.path().to_owned());
        }
    }
}
