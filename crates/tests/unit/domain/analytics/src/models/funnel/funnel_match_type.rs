//! Tests for funnel match type and step input types.

use systemprompt_analytics::{CreateFunnelStepInput, FunnelMatchType};

mod funnel_match_type_tests {
    use super::*;

    #[test]
    fn match_types_are_different() {
        assert_ne!(FunnelMatchType::UrlExact, FunnelMatchType::UrlPrefix);
        assert_ne!(FunnelMatchType::UrlPrefix, FunnelMatchType::UrlRegex);
        assert_ne!(FunnelMatchType::UrlRegex, FunnelMatchType::EventType);
        assert_ne!(FunnelMatchType::EventType, FunnelMatchType::UrlExact);
    }

    #[test]
    fn match_type_is_debug() {
        let debug_str = format!("{:?}", FunnelMatchType::EventType);
        assert!(debug_str.contains("EventType"));
    }

    #[test]
    fn match_type_serializes_url_exact() {
        let json = serde_json::to_string(&FunnelMatchType::UrlExact).unwrap();
        assert!(json.contains("url_exact"));
    }

    #[test]
    fn match_type_serializes_url_prefix() {
        let json = serde_json::to_string(&FunnelMatchType::UrlPrefix).unwrap();
        assert!(json.contains("url_prefix"));
    }

    #[test]
    fn match_type_serializes_url_regex() {
        let json = serde_json::to_string(&FunnelMatchType::UrlRegex).unwrap();
        assert!(json.contains("url_regex"));
    }

    #[test]
    fn match_type_serializes_event_type() {
        let json = serde_json::to_string(&FunnelMatchType::EventType).unwrap();
        assert!(json.contains("event_type"));
    }
}

mod create_funnel_step_input_tests {
    use super::*;

    fn create_step(name: &str, pattern: &str, match_type: FunnelMatchType) -> CreateFunnelStepInput {
        CreateFunnelStepInput {
            name: name.to_string(),
            match_pattern: pattern.to_string(),
            match_type,
        }
    }

    #[test]
    fn step_stores_name() {
        let step = create_step("Landing Page", "/landing", FunnelMatchType::UrlExact);
        assert_eq!(step.name, "Landing Page");
    }

    #[test]
    fn step_stores_match_pattern() {
        let step = create_step("Blog", "/blog/*", FunnelMatchType::UrlPrefix);
        assert_eq!(step.match_pattern, "/blog/*");
    }

    #[test]
    fn step_stores_match_type() {
        let step = create_step("Conversion", "conversion", FunnelMatchType::EventType);
        assert_eq!(step.match_type, FunnelMatchType::EventType);
    }

    #[test]
    fn step_is_debug() {
        let step = create_step("Debug", "/debug", FunnelMatchType::UrlPrefix);
        let debug_str = format!("{:?}", step);
        assert!(debug_str.contains("CreateFunnelStepInput"));
    }

    #[test]
    fn step_serializes() {
        let step = create_step("Signup", "/signup", FunnelMatchType::UrlExact);
        let json = serde_json::to_string(&step).unwrap();

        assert!(json.contains("Signup"));
        assert!(json.contains("/signup"));
        assert!(json.contains("url_exact"));
    }
}
