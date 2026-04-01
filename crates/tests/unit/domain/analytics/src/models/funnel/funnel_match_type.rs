//! Tests for funnel match type and step input types.

use systemprompt_analytics::{CreateFunnelStepInput, FunnelMatchType};

mod funnel_match_type_tests {
    use super::*;

    #[test]
    fn url_exact_is_eq() {
        assert_eq!(FunnelMatchType::UrlExact, FunnelMatchType::UrlExact);
    }

    #[test]
    fn url_prefix_is_eq() {
        assert_eq!(FunnelMatchType::UrlPrefix, FunnelMatchType::UrlPrefix);
    }

    #[test]
    fn url_regex_is_eq() {
        assert_eq!(FunnelMatchType::UrlRegex, FunnelMatchType::UrlRegex);
    }

    #[test]
    fn event_type_is_eq() {
        assert_eq!(FunnelMatchType::EventType, FunnelMatchType::EventType);
    }

    #[test]
    fn match_types_are_different() {
        assert_ne!(FunnelMatchType::UrlExact, FunnelMatchType::UrlPrefix);
        assert_ne!(FunnelMatchType::UrlPrefix, FunnelMatchType::UrlRegex);
        assert_ne!(FunnelMatchType::UrlRegex, FunnelMatchType::EventType);
        assert_ne!(FunnelMatchType::EventType, FunnelMatchType::UrlExact);
    }

    #[test]
    fn match_type_is_copy() {
        let match_type = FunnelMatchType::UrlPrefix;
        let copied = match_type;
        assert_eq!(match_type, copied);
    }

    #[test]
    fn match_type_is_clone() {
        let match_type = FunnelMatchType::UrlRegex;
        let cloned = match_type.clone();
        assert_eq!(match_type, cloned);
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

    #[test]
    fn match_type_deserializes_url_exact() {
        let json = r#""url_exact""#;
        let match_type: FunnelMatchType = serde_json::from_str(json).unwrap();
        assert_eq!(match_type, FunnelMatchType::UrlExact);
    }

    #[test]
    fn match_type_deserializes_url_prefix() {
        let json = r#""url_prefix""#;
        let match_type: FunnelMatchType = serde_json::from_str(json).unwrap();
        assert_eq!(match_type, FunnelMatchType::UrlPrefix);
    }

    #[test]
    fn match_type_deserializes_url_regex() {
        let json = r#""url_regex""#;
        let match_type: FunnelMatchType = serde_json::from_str(json).unwrap();
        assert_eq!(match_type, FunnelMatchType::UrlRegex);
    }

    #[test]
    fn match_type_deserializes_event_type() {
        let json = r#""event_type""#;
        let match_type: FunnelMatchType = serde_json::from_str(json).unwrap();
        assert_eq!(match_type, FunnelMatchType::EventType);
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
    fn step_is_clone() {
        let step = create_step("Test", "/test", FunnelMatchType::UrlExact);
        let cloned = step.clone();

        assert_eq!(step.name, cloned.name);
        assert_eq!(step.match_pattern, cloned.match_pattern);
        assert_eq!(step.match_type, cloned.match_type);
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

    #[test]
    fn step_deserializes() {
        let json = r#"{
            "name": "Checkout",
            "match_pattern": "/checkout",
            "match_type": "url_exact"
        }"#;

        let step: CreateFunnelStepInput = serde_json::from_str(json).unwrap();

        assert_eq!(step.name, "Checkout");
        assert_eq!(step.match_pattern, "/checkout");
        assert_eq!(step.match_type, FunnelMatchType::UrlExact);
    }

    #[test]
    fn step_deserializes_with_regex() {
        let json = r#"{
            "name": "Product Page",
            "match_pattern": "^/products/[a-z0-9-]+$",
            "match_type": "url_regex"
        }"#;

        let step: CreateFunnelStepInput = serde_json::from_str(json).unwrap();

        assert_eq!(step.match_type, FunnelMatchType::UrlRegex);
        assert!(step.match_pattern.starts_with('^'));
    }
}
