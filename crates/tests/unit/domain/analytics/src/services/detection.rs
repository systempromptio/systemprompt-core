//! Tests for static detection helpers: datacenter IPs, spam referrers,
//! high-risk countries.

use systemprompt_analytics::detection::{
    is_datacenter_ip, is_high_risk_country, is_spam_referrer, DATACENTER_IP_PREFIXES,
    HIGH_RISK_COUNTRIES, SPAM_REFERRER_PATTERNS,
};

mod is_datacenter_ip_tests {
    use super::*;

    #[test]
    fn known_prefix_47_79_is_datacenter() {
        assert!(is_datacenter_ip("47.79.1.1"));
    }

    #[test]
    fn known_prefix_47_88_is_datacenter() {
        assert!(is_datacenter_ip("47.88.100.200"));
    }

    #[test]
    fn known_prefix_119_29_is_datacenter() {
        assert!(is_datacenter_ip("119.29.0.1"));
    }

    #[test]
    fn known_prefix_114_116_is_datacenter() {
        assert!(is_datacenter_ip("114.116.50.50"));
    }

    #[test]
    fn residential_ip_is_not_datacenter() {
        assert!(!is_datacenter_ip("192.168.1.1"));
    }

    #[test]
    fn loopback_is_not_datacenter() {
        assert!(!is_datacenter_ip("127.0.0.1"));
    }

    #[test]
    fn empty_string_is_not_datacenter() {
        assert!(!is_datacenter_ip(""));
    }

    #[test]
    fn partial_prefix_match_is_not_datacenter() {
        assert!(!is_datacenter_ip("47.1.1.1"));
    }

    #[test]
    fn all_datacenter_prefixes_match_themselves() {
        for prefix in DATACENTER_IP_PREFIXES {
            let ip = format!("{prefix}1");
            assert!(
                is_datacenter_ip(&ip),
                "prefix {prefix} should match ip {ip}"
            );
        }
    }

    #[test]
    fn known_prefix_49_51_is_datacenter() {
        assert!(is_datacenter_ip("49.51.200.100"));
    }

    #[test]
    fn known_prefix_122_112_is_datacenter() {
        assert!(is_datacenter_ip("122.112.1.2"));
    }

    #[test]
    fn ipv6_style_string_is_not_datacenter() {
        assert!(!is_datacenter_ip("::1"));
    }
}

mod is_spam_referrer_tests {
    use super::*;

    #[test]
    fn buttons_for_website_is_spam() {
        assert!(is_spam_referrer("http://buttons-for-website.com/"));
    }

    #[test]
    fn darodar_is_spam() {
        assert!(is_spam_referrer("http://darodar.com/"));
    }

    #[test]
    fn best_seo_solution_is_spam() {
        assert!(is_spam_referrer("https://best-seo-solution.com/free-tools"));
    }

    #[test]
    fn free_social_buttons_is_spam() {
        assert!(is_spam_referrer("http://free-social-buttons.com"));
    }

    #[test]
    fn get_free_traffic_now_is_spam() {
        assert!(is_spam_referrer("http://get-free-traffic-now.com/"));
    }

    #[test]
    fn legitimate_google_is_not_spam() {
        assert!(!is_spam_referrer("https://google.com/search?q=rust"));
    }

    #[test]
    fn empty_referrer_is_not_spam() {
        assert!(!is_spam_referrer(""));
    }

    #[test]
    fn case_insensitive_matching() {
        assert!(is_spam_referrer("http://DARODAR.COM/"));
    }

    #[test]
    fn all_patterns_match() {
        for pattern in SPAM_REFERRER_PATTERNS {
            assert!(
                is_spam_referrer(&format!("http://{pattern}.com/")),
                "pattern {pattern} should be detected"
            );
        }
    }

    #[test]
    fn legitimate_referrer_with_similar_text_is_not_spam() {
        assert!(!is_spam_referrer("https://social-buttons-platform.io/"));
    }
}

mod is_high_risk_country_tests {
    use super::*;

    #[test]
    fn russia_is_high_risk() {
        assert!(is_high_risk_country("RU"));
    }

    #[test]
    fn china_is_high_risk() {
        assert!(is_high_risk_country("CN"));
    }

    #[test]
    fn brazil_is_high_risk() {
        assert!(is_high_risk_country("BR"));
    }

    #[test]
    fn vietnam_is_high_risk() {
        assert!(is_high_risk_country("VN"));
    }

    #[test]
    fn nigeria_is_high_risk() {
        assert!(is_high_risk_country("NG"));
    }

    #[test]
    fn us_is_not_high_risk() {
        assert!(!is_high_risk_country("US"));
    }

    #[test]
    fn uk_is_not_high_risk() {
        assert!(!is_high_risk_country("GB"));
    }

    #[test]
    fn germany_is_not_high_risk() {
        assert!(!is_high_risk_country("DE"));
    }

    #[test]
    fn empty_string_is_not_high_risk() {
        assert!(!is_high_risk_country(""));
    }

    #[test]
    fn lowercase_does_not_match() {
        assert!(!is_high_risk_country("ru"));
    }

    #[test]
    fn all_high_risk_countries_in_list() {
        for country in HIGH_RISK_COUNTRIES {
            assert!(
                is_high_risk_country(country),
                "country {country} should be high risk"
            );
        }
    }

    #[test]
    fn singapore_is_high_risk() {
        assert!(is_high_risk_country("SG"));
    }

    #[test]
    fn iran_is_high_risk() {
        assert!(is_high_risk_country("IR"));
    }
}
