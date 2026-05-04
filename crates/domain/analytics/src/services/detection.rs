//! Static lookup tables and predicates for spam-referrer, datacenter-IP and
//! high-risk-country detection used by the analytics ingestion pipeline.

/// Substrings that identify referrer URLs from known spam farms.
pub const SPAM_REFERRER_PATTERNS: &[&str] = &[
    "tyingshoelaces",
    "buttons-for-website",
    "darodar",
    "best-seo-solution",
    "free-social-buttons",
    "get-free-traffic-now",
];

/// Leading octets that identify well-known datacenter / VPN IP ranges.
pub const DATACENTER_IP_PREFIXES: &[&str] = &[
    "47.79.",
    "47.82.",
    "14.22.49.",
    "47.88.",
    "47.89.",
    "47.90.",
    "47.91.",
    "47.74.",
    "47.75.",
    "47.76.",
    "119.29.",
    "129.28.",
    "49.51.",
    "119.3.",
    "114.116.",
    "122.112.",
];

/// ISO 3166-1 alpha-2 codes for countries that historically generate a
/// disproportionate share of abusive traffic.
pub const HIGH_RISK_COUNTRIES: &[&str] = &[
    "BR", "VN", "AR", "IQ", "BD", "PK", "RU", "VE", "TH", "UA", "ID", "MY", "PH", "NG", "KE", "EG",
    "MA", "DZ", "TN", "LY", "SY", "IR", "AF", "MM", "KH", "LA", "NP", "LK", "KZ", "UZ", "AZ", "GE",
    "CN", "SG",
];

/// Returns `true` when `ip` starts with one of [`DATACENTER_IP_PREFIXES`].
pub fn is_datacenter_ip(ip: &str) -> bool {
    DATACENTER_IP_PREFIXES
        .iter()
        .any(|prefix| ip.starts_with(prefix))
}

/// Returns `true` when `country` is in [`HIGH_RISK_COUNTRIES`].
pub fn is_high_risk_country(country: &str) -> bool {
    HIGH_RISK_COUNTRIES.contains(&country)
}

/// Returns `true` when the lowercased `url` contains one of
/// [`SPAM_REFERRER_PATTERNS`].
pub fn is_spam_referrer(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    SPAM_REFERRER_PATTERNS
        .iter()
        .any(|pattern| url_lower.contains(pattern))
}
