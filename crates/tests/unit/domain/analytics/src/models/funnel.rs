//! Tests for funnel model types.

use systemprompt_analytics::{
    CreateFunnelInput, CreateFunnelStepInput, FunnelMatchType, FunnelStats, FunnelStepStats,
    FunnelWithSteps,
};
use systemprompt_identifiers::FunnelId;

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

mod create_funnel_input_tests {
    use super::*;

    fn create_funnel(name: &str, desc: Option<&str>, steps: Vec<CreateFunnelStepInput>) -> CreateFunnelInput {
        CreateFunnelInput {
            name: name.to_string(),
            description: desc.map(|s| s.to_string()),
            steps,
        }
    }

    fn sample_step() -> CreateFunnelStepInput {
        CreateFunnelStepInput {
            name: "Step 1".to_string(),
            match_pattern: "/step1".to_string(),
            match_type: FunnelMatchType::UrlExact,
        }
    }

    #[test]
    fn funnel_stores_name() {
        let funnel = create_funnel("Signup Funnel", None, vec![]);
        assert_eq!(funnel.name, "Signup Funnel");
    }

    #[test]
    fn funnel_stores_description() {
        let funnel = create_funnel("Test", Some("A test funnel"), vec![]);
        assert_eq!(funnel.description, Some("A test funnel".to_string()));
    }

    #[test]
    fn funnel_stores_steps() {
        let steps = vec![
            CreateFunnelStepInput {
                name: "Landing".to_string(),
                match_pattern: "/landing".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            CreateFunnelStepInput {
                name: "Signup".to_string(),
                match_pattern: "/signup".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            CreateFunnelStepInput {
                name: "Complete".to_string(),
                match_pattern: "/complete".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
        ];

        let funnel = create_funnel("Registration", None, steps);

        assert_eq!(funnel.steps.len(), 3);
        assert_eq!(funnel.steps[0].name, "Landing");
        assert_eq!(funnel.steps[2].name, "Complete");
    }

    #[test]
    fn funnel_with_no_description() {
        let funnel = create_funnel("Simple", None, vec![sample_step()]);
        assert!(funnel.description.is_none());
    }

    #[test]
    fn funnel_is_clone() {
        let funnel = create_funnel("Clone Test", Some("Test"), vec![sample_step()]);
        let cloned = funnel.clone();

        assert_eq!(funnel.name, cloned.name);
        assert_eq!(funnel.description, cloned.description);
        assert_eq!(funnel.steps.len(), cloned.steps.len());
    }

    #[test]
    fn funnel_is_debug() {
        let funnel = create_funnel("Debug Test", None, vec![]);
        let debug_str = format!("{:?}", funnel);
        assert!(debug_str.contains("CreateFunnelInput"));
    }

    #[test]
    fn funnel_serializes() {
        let funnel = create_funnel("Serialize Test", Some("desc"), vec![sample_step()]);
        let json = serde_json::to_string(&funnel).unwrap();

        assert!(json.contains("Serialize Test"));
        assert!(json.contains("desc"));
        assert!(json.contains("steps"));
    }

    #[test]
    fn funnel_deserializes() {
        let json = r#"{
            "name": "Purchase Funnel",
            "description": "Track purchases",
            "steps": [
                {"name": "View", "match_pattern": "/products", "match_type": "url_prefix"},
                {"name": "Add to Cart", "match_pattern": "add_to_cart", "match_type": "event_type"},
                {"name": "Checkout", "match_pattern": "/checkout", "match_type": "url_exact"}
            ]
        }"#;

        let funnel: CreateFunnelInput = serde_json::from_str(json).unwrap();

        assert_eq!(funnel.name, "Purchase Funnel");
        assert_eq!(funnel.description, Some("Track purchases".to_string()));
        assert_eq!(funnel.steps.len(), 3);
        assert_eq!(funnel.steps[1].match_type, FunnelMatchType::EventType);
    }

    #[test]
    fn funnel_deserializes_minimal() {
        let json = r#"{"name": "Minimal", "steps": []}"#;
        let funnel: CreateFunnelInput = serde_json::from_str(json).unwrap();

        assert_eq!(funnel.name, "Minimal");
        assert!(funnel.description.is_none());
        assert!(funnel.steps.is_empty());
    }
}

mod funnel_step_stats_tests {
    use super::*;

    fn create_stats(
        step_order: i32,
        entered: i64,
        exited: i64,
        rate: f64,
        avg_time: Option<i64>,
    ) -> FunnelStepStats {
        FunnelStepStats {
            step_order,
            entered_count: entered,
            exited_count: exited,
            conversion_rate: rate,
            avg_time_to_next_ms: avg_time,
        }
    }

    #[test]
    fn stats_stores_step_order() {
        let stats = create_stats(1, 100, 90, 0.9, Some(5000));
        assert_eq!(stats.step_order, 1);
    }

    #[test]
    fn stats_stores_entered_count() {
        let stats = create_stats(0, 500, 400, 0.8, None);
        assert_eq!(stats.entered_count, 500);
    }

    #[test]
    fn stats_stores_exited_count() {
        let stats = create_stats(2, 100, 75, 0.75, Some(3000));
        assert_eq!(stats.exited_count, 75);
    }

    #[test]
    fn stats_stores_conversion_rate() {
        let stats = create_stats(1, 100, 50, 0.5, None);
        assert!((stats.conversion_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_stores_avg_time() {
        let stats = create_stats(1, 100, 90, 0.9, Some(10000));
        assert_eq!(stats.avg_time_to_next_ms, Some(10000));
    }

    #[test]
    fn stats_handles_no_avg_time() {
        let stats = create_stats(3, 50, 0, 0.0, None);
        assert!(stats.avg_time_to_next_ms.is_none());
    }

    #[test]
    fn stats_is_copy() {
        let stats = create_stats(1, 100, 90, 0.9, Some(5000));
        let copied = stats;
        assert_eq!(stats.step_order, copied.step_order);
    }

    #[test]
    fn stats_is_clone() {
        let stats = create_stats(2, 200, 180, 0.9, Some(3000));
        let cloned = stats.clone();
        assert_eq!(stats.entered_count, cloned.entered_count);
    }

    #[test]
    fn stats_is_debug() {
        let stats = create_stats(0, 100, 100, 1.0, None);
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("FunnelStepStats"));
    }

    #[test]
    fn stats_serializes() {
        let stats = create_stats(1, 100, 75, 0.75, Some(2500));
        let json = serde_json::to_string(&stats).unwrap();

        assert!(json.contains("step_order"));
        assert!(json.contains("entered_count"));
        assert!(json.contains("conversion_rate"));
    }

    #[test]
    fn stats_deserializes() {
        let json = r#"{
            "step_order": 2,
            "entered_count": 500,
            "exited_count": 400,
            "conversion_rate": 0.8,
            "avg_time_to_next_ms": 1500
        }"#;

        let stats: FunnelStepStats = serde_json::from_str(json).unwrap();

        assert_eq!(stats.step_order, 2);
        assert_eq!(stats.entered_count, 500);
        assert_eq!(stats.exited_count, 400);
        assert!((stats.conversion_rate - 0.8).abs() < f64::EPSILON);
        assert_eq!(stats.avg_time_to_next_ms, Some(1500));
    }

    #[test]
    fn stats_deserializes_without_avg_time() {
        let json = r#"{
            "step_order": 0,
            "entered_count": 1000,
            "exited_count": 800,
            "conversion_rate": 0.8,
            "avg_time_to_next_ms": null
        }"#;

        let stats: FunnelStepStats = serde_json::from_str(json).unwrap();
        assert!(stats.avg_time_to_next_ms.is_none());
    }
}

mod funnel_stats_tests {
    use super::*;

    fn create_funnel_stats(
        funnel_id: &str,
        name: &str,
        entries: i64,
        completions: i64,
        rate: f64,
    ) -> FunnelStats {
        FunnelStats {
            funnel_id: FunnelId::new(funnel_id.to_string()),
            funnel_name: name.to_string(),
            total_entries: entries,
            total_completions: completions,
            overall_conversion_rate: rate,
            step_stats: vec![],
        }
    }

    #[test]
    fn stats_stores_funnel_id() {
        let stats = create_funnel_stats("fnl_123", "Test", 100, 50, 0.5);
        assert_eq!(stats.funnel_id.as_str(), "fnl_123");
    }

    #[test]
    fn stats_stores_funnel_name() {
        let stats = create_funnel_stats("fnl_456", "Purchase Funnel", 200, 50, 0.25);
        assert_eq!(stats.funnel_name, "Purchase Funnel");
    }

    #[test]
    fn stats_stores_total_entries() {
        let stats = create_funnel_stats("fnl_1", "Test", 1000, 100, 0.1);
        assert_eq!(stats.total_entries, 1000);
    }

    #[test]
    fn stats_stores_total_completions() {
        let stats = create_funnel_stats("fnl_2", "Test", 500, 250, 0.5);
        assert_eq!(stats.total_completions, 250);
    }

    #[test]
    fn stats_stores_overall_conversion_rate() {
        let stats = create_funnel_stats("fnl_3", "Test", 400, 100, 0.25);
        assert!((stats.overall_conversion_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_with_step_stats() {
        let step_stats = vec![
            FunnelStepStats {
                step_order: 0,
                entered_count: 1000,
                exited_count: 800,
                conversion_rate: 0.8,
                avg_time_to_next_ms: Some(5000),
            },
            FunnelStepStats {
                step_order: 1,
                entered_count: 800,
                exited_count: 500,
                conversion_rate: 0.625,
                avg_time_to_next_ms: Some(10000),
            },
            FunnelStepStats {
                step_order: 2,
                entered_count: 500,
                exited_count: 500,
                conversion_rate: 1.0,
                avg_time_to_next_ms: None,
            },
        ];

        let stats = FunnelStats {
            funnel_id: FunnelId::new("fnl_full".to_string()),
            funnel_name: "Full Funnel".to_string(),
            total_entries: 1000,
            total_completions: 500,
            overall_conversion_rate: 0.5,
            step_stats,
        };

        assert_eq!(stats.step_stats.len(), 3);
        assert_eq!(stats.step_stats[0].entered_count, 1000);
        assert_eq!(stats.step_stats[2].step_order, 2);
    }

    #[test]
    fn stats_is_clone() {
        let stats = create_funnel_stats("fnl_clone", "Clone Test", 100, 50, 0.5);
        let cloned = stats.clone();

        assert_eq!(stats.funnel_name, cloned.funnel_name);
        assert_eq!(stats.total_entries, cloned.total_entries);
    }

    #[test]
    fn stats_is_debug() {
        let stats = create_funnel_stats("fnl_debug", "Debug Test", 50, 25, 0.5);
        let debug_str = format!("{:?}", stats);
        assert!(debug_str.contains("FunnelStats"));
    }

    #[test]
    fn stats_serializes() {
        let stats = create_funnel_stats("fnl_ser", "Serialize", 100, 75, 0.75);
        let json = serde_json::to_string(&stats).unwrap();

        assert!(json.contains("funnel_id"));
        assert!(json.contains("fnl_ser"));
        assert!(json.contains("Serialize"));
        assert!(json.contains("overall_conversion_rate"));
    }

    #[test]
    fn stats_deserializes() {
        let json = r#"{
            "funnel_id": "fnl_deser",
            "funnel_name": "Deserialized",
            "total_entries": 1000,
            "total_completions": 250,
            "overall_conversion_rate": 0.25,
            "step_stats": []
        }"#;

        let stats: FunnelStats = serde_json::from_str(json).unwrap();

        assert_eq!(stats.funnel_id.as_str(), "fnl_deser");
        assert_eq!(stats.funnel_name, "Deserialized");
        assert_eq!(stats.total_entries, 1000);
        assert_eq!(stats.total_completions, 250);
        assert!(stats.step_stats.is_empty());
    }

    #[test]
    fn stats_handles_zero_entries() {
        let stats = create_funnel_stats("fnl_empty", "Empty", 0, 0, 0.0);

        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_completions, 0);
        assert!((stats.overall_conversion_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_handles_perfect_conversion() {
        let stats = create_funnel_stats("fnl_perfect", "Perfect", 100, 100, 1.0);

        assert_eq!(stats.total_entries, 100);
        assert_eq!(stats.total_completions, 100);
        assert!((stats.overall_conversion_rate - 1.0).abs() < f64::EPSILON);
    }
}

mod funnel_with_steps_tests {
    use super::*;
    use chrono::Utc;
    use systemprompt_analytics::{Funnel, FunnelStep};

    fn create_funnel_with_steps() -> FunnelWithSteps {
        let now = Utc::now();
        let funnel = Funnel {
            id: FunnelId::new("fnl_test".to_string()),
            name: "Test Funnel".to_string(),
            description: Some("A test funnel".to_string()),
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let steps = vec![
            FunnelStep {
                funnel_id: FunnelId::new("fnl_test".to_string()),
                step_order: 0,
                name: "Step 1".to_string(),
                match_pattern: "/step1".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
            FunnelStep {
                funnel_id: FunnelId::new("fnl_test".to_string()),
                step_order: 1,
                name: "Step 2".to_string(),
                match_pattern: "/step2".to_string(),
                match_type: FunnelMatchType::UrlExact,
            },
        ];

        FunnelWithSteps { funnel, steps }
    }

    #[test]
    fn funnel_with_steps_stores_funnel() {
        let fws = create_funnel_with_steps();
        assert_eq!(fws.funnel.name, "Test Funnel");
        assert!(fws.funnel.is_active);
    }

    #[test]
    fn funnel_with_steps_stores_steps() {
        let fws = create_funnel_with_steps();
        assert_eq!(fws.steps.len(), 2);
        assert_eq!(fws.steps[0].name, "Step 1");
        assert_eq!(fws.steps[1].step_order, 1);
    }

    #[test]
    fn funnel_with_steps_is_clone() {
        let fws = create_funnel_with_steps();
        let cloned = fws.clone();

        assert_eq!(fws.funnel.name, cloned.funnel.name);
        assert_eq!(fws.steps.len(), cloned.steps.len());
    }

    #[test]
    fn funnel_with_steps_is_debug() {
        let fws = create_funnel_with_steps();
        let debug_str = format!("{:?}", fws);
        assert!(debug_str.contains("FunnelWithSteps"));
    }

    #[test]
    fn funnel_with_steps_serializes() {
        let fws = create_funnel_with_steps();
        let json = serde_json::to_string(&fws).unwrap();

        assert!(json.contains("Test Funnel"));
        assert!(json.contains("Step 1"));
        assert!(json.contains("Step 2"));
    }
}
