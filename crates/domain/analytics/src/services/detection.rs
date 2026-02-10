pub const SPAM_REFERRER_PATTERNS: &[&str] = &[
    "tyingshoelaces",
    "buttons-for-website",
    "darodar",
    "best-seo-solution",
    "free-social-buttons",
    "get-free-traffic-now",
];

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

pub const HIGH_RISK_COUNTRIES: &[&str] = &[
    "BR", "VN", "AR", "IQ", "BD", "PK", "RU", "VE", "TH", "UA", "ID", "MY", "PH", "NG", "KE", "EG",
    "MA", "DZ", "TN", "LY", "SY", "IR", "AF", "MM", "KH", "LA", "NP", "LK", "KZ", "UZ", "AZ", "GE",
    "CN", "SG",
];

pub fn is_datacenter_ip(ip: &str) -> bool {
    DATACENTER_IP_PREFIXES
        .iter()
        .any(|prefix| ip.starts_with(prefix))
}

pub fn is_high_risk_country(country: &str) -> bool {
    HIGH_RISK_COUNTRIES.contains(&country)
}

pub fn is_spam_referrer(url: &str) -> bool {
    let url_lower = url.to_lowercase();
    SPAM_REFERRER_PATTERNS
        .iter()
        .any(|pattern| url_lower.contains(pattern))
}
